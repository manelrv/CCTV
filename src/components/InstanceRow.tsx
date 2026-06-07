import { useState } from "react";
import { useTranslation } from "react-i18next";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { invoke } from "@tauri-apps/api/core";
import type { Instance } from "../types";
import { STATE_CLASS, STATE_I18N_KEY, copyPayload, formatTokens, tokenLevel } from "../types";

function timeInState(lastEventAt: number, now: number): string {
  const secs = Math.max(0, now - lastEventAt);
  const m = Math.floor(secs / 60);
  // From one hour up, mm:ss becomes unreadable (1038:19) — switch to "17h 18m".
  if (m >= 60) {
    const h = Math.floor(m / 60);
    return `${h}h ${m % 60}m`;
  }
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function InstanceRow({ inst, now }: { inst: Instance; now: number }) {
  const { t } = useTranslation();
  const cls = STATE_CLASS[inst.state];
  const [copied, setCopied] = useState(false);

  /**
   * Click behavior:
   *   - Alt+click: always copy (regardless of other conditions).
   *   - Background source: copy (no terminal to focus).
   *   - Foreground with no terminal info: copy (script hook not yet received).
   *   - Foreground with terminal info: invoke focus_session; if it returns false
   *     (focus failed), fall back to copy. No visual feedback on focus success —
   *     the terminal coming to front IS the feedback.
   */
  async function handleClick(e: React.MouseEvent) {
    const shouldCopy =
      e.altKey || inst.source === "background" || !inst.terminal;

    if (shouldCopy) {
      const payload = copyPayload(inst);
      writeText(payload).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      });
      return;
    }

    // Foreground row with terminal info — attempt focus.
    try {
      const focused = await invoke<boolean>("focus_session", {
        sessionId: inst.session_id,
      });
      if (!focused) {
        // Focus failed (e.g. non-macOS, or osascript error) — fall back to copy.
        const payload = copyPayload(inst);
        writeText(payload).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1200);
        });
      }
      // On success: no feedback — the terminal window coming to front is sufficient.
    } catch {
      // Tauri command error — fall back to copy.
      const payload = copyPayload(inst);
      writeText(payload).then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      });
    }
  }

  return (
    <div className={`row ${cls}`} onClick={(e) => void handleClick(e)} style={{ cursor: "pointer" }}>
      <span className="dot" aria-hidden />
      <div className="row-main">
        <div className="project">
          {inst.project}
          <span className="source-badge">{inst.source === "background" ? "bg" : "fg"}</span>
          {inst.in_flight_tasks != null && inst.in_flight_tasks > 0 && (
            <span className="inflight-badge">⚙ {inst.in_flight_tasks}</span>
          )}
        </div>
        {inst.detail && <div className="detail">{inst.detail}</div>}
      </div>
      <div className="row-meta">
        <div className="state">{copied ? t("copied") : t(STATE_I18N_KEY[inst.state])}</div>
        <div className="time-tokens">
          <span className="time">{timeInState(inst.last_event_at, now)}</span>
          {inst.context_tokens != null && (
            <span className={`ctx-tokens ctx-${tokenLevel(inst.context_tokens)}`}>
              {formatTokens(inst.context_tokens)}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
