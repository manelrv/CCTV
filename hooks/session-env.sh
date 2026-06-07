#!/usr/bin/env bash
# session-env.sh — enriches the Claude Code hook payload with terminal env info
# and POSTs it to the CCTV app endpoint.
#
# Usage: ./session-env.sh <endpoint-url>
#   e.g. ./session-env.sh http://127.0.0.1:8787/hooks/session-start
#
# This script is invoked by Claude Code as a "command"-type hook. It runs as a
# child of the claude process and inherits its environment — that is how we read
# TERM_PROGRAM, ITERM_SESSION_ID, TERM_SESSION_ID, and the parent process tty.
#
# SAFETY CONTRACT (must never break):
#   - Always exits 0 — a non-zero exit marks the hook as failed (user-visible).
#   - curl has a hard 2-second timeout (-m 2). If the app is down, silent no-op.
#   - python3 is used for JSON safety (always present on macOS). The payload and
#     all env values are passed via environment variables (never shell-interpolated
#     into python source) to prevent injection through crafted JSON field values.

ENDPOINT="${1:-}"
if [ -z "$ENDPOINT" ]; then
  exit 0
fi

# Read the hook payload from stdin (Claude Code writes it there).
PAYLOAD=$(cat 2>/dev/null) || true
if [ -z "$PAYLOAD" ]; then
  exit 0
fi

# Collect terminal environment fields.
TERM_PROG="${TERM_PROGRAM:-}"

# Prefer iTerm's per-tab session id; fall back to the generic TERM_SESSION_ID.
TERM_SID="${ITERM_SESSION_ID:-${TERM_SESSION_ID:-}}"

# Derive the tty from the parent process (the claude binary).
# ps -o tty= -p PPID prints just the tty column with no header (e.g. "ttys003").
# May be "??" for detached/daemon processes — normalize that to empty.
TTY_RAW=$(ps -o tty= -p $PPID 2>/dev/null | tr -d '[:space:]') || TTY_RAW=""
[ "$TTY_RAW" = "??" ] && TTY_RAW=""

# Terminal focus deep link (Warp: warp://session/<32hex> or warposs://session/<32hex>).
# Other terminals may expose a similar var in future — add them here.
FOCUS_URL="${WARP_FOCUS_URL:-}"

# Pass payload and env values via environment variables into python3 so that
# no shell interpolation touches JSON or tty strings (injection-safe).
ENRICHED=$(
  _CCTV_PAYLOAD="$PAYLOAD" \
  _CCTV_TERM_PROG="$TERM_PROG" \
  _CCTV_TERM_SID="$TERM_SID" \
  _CCTV_TTY="$TTY_RAW" \
  _CCTV_FOCUS_URL="$FOCUS_URL" \
  python3 -c '
import json, sys, os

raw = os.environ.get("_CCTV_PAYLOAD", "")
if not raw:
    sys.exit(0)

try:
    payload = json.loads(raw)
except Exception:
    # Cannot parse — emit original unchanged so the event still arrives.
    print(raw)
    sys.exit(0)

def nonempty(k):
    v = os.environ.get(k, "")
    return v if v else None

payload["term_program"]    = nonempty("_CCTV_TERM_PROG")
payload["term_session_id"] = nonempty("_CCTV_TERM_SID")
payload["tty"]             = nonempty("_CCTV_TTY")
payload["focus_url"]       = nonempty("_CCTV_FOCUS_URL")

print(json.dumps(payload))
' 2>/dev/null
) || ENRICHED=""

# If enrichment failed entirely, fall back to the original payload so the event
# still reaches the app (just without terminal info).
[ -z "$ENRICHED" ] && ENRICHED="$PAYLOAD"

# POST to the CCTV app. Silent (-s), max 2 s (-m 2). If the app is down: no-op.
curl -s -m 2 \
  -X POST \
  -H "Content-Type: application/json" \
  -d "$ENRICHED" \
  "$ENDPOINT" >/dev/null 2>&1 || true

exit 0
