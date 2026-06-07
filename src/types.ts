/** TypeScript mirror of config::Prefs in Rust. Serialized by serde with snake_case. */
export interface Prefs {
  floating_window: boolean;
  always_on_top: boolean;
  auto_hide: boolean;
  compact: boolean;
  open_at_login: boolean;
}

export type InstanceState =
  | "working"
  | "waiting_permission"
  | "waiting_input"
  | "error"
  | "unknown"
  | "idle"
  | "completed";

export type Source = "background" | "foreground";

export interface Instance {
  session_id: string;
  cwd: string;
  project: string;
  state: InstanceState;
  detail: string | null;
  source: Source;
  started_at: number; // epoch secs
  last_event_at: number; // epoch secs
  /** Sum of input + cache_read + cache_creation tokens from the last assistant message. */
  context_tokens: number | null;
  /** Background only: subtasks in flight (tasks + queued). Null for foreground. */
  in_flight_tasks: number | null;
  /** Terminal hosting this session. Foreground only; null for background or before the first enriched hook. */
  terminal: {
    program: string;
    session_id: string | null;
    tty: string | null;
    /** Deep link for exact-pane focus (e.g. "warp://session/<32hex>"). Null when not available. */
    focus_url: string | null;
  } | null;
}

/**
 * Returns the text that should be copied to clipboard when the user clicks a row.
 * - background source: `claude attach <shortId>` where shortId is the first UUID segment
 *   (e.g. "be4c186b-1eb4-..." → "be4c186b"; verified: daemonShort == first UUID segment).
 * - foreground source: the instance working directory (cwd).
 */
export function copyPayload(inst: Instance): string {
  if (inst.source === "background") {
    const shortId = inst.session_id.split("-")[0];
    return `claude attach ${shortId}`;
  }
  return inst.cwd;
}

/**
 * Formats a raw token count into a compact label.
 * < 1000 → as-is (e.g. "42")
 * ≥ 1000 → rounded to nearest k (e.g. "304k")
 */
export function formatTokens(n: number): string {
  if (n < 1000) return String(n);
  return `${Math.round(n / 1000)}k`;
}

/**
 * Severity of the context occupancy relative to the inferred window.
 *
 * The transcript's model field does not distinguish 200k from 1M context
 * variants (verified empirically: a 307k-token session reports plain
 * "claude-opus-4-8"). Heuristic: tokens above 200k imply a 1M window; since
 * context only grows within a session, the inference is monotonic.
 * Known limitation: a 1M session between 150k and 200k shows an early
 * warning until it crosses 200k.
 */
export function tokenLevel(n: number): "ok" | "warn" | "crit" {
  const window = n > 200_000 ? 1_000_000 : 200_000;
  const ratio = n / window;
  if (ratio >= 0.9) return "crit";
  if (ratio >= 0.75) return "warn";
  return "ok";
}

// Translation keys for each state (resolved via t() in the component).
export const STATE_I18N_KEY: Record<InstanceState, string> = {
  working: "state.working",
  waiting_permission: "state.waiting_permission",
  waiting_input: "state.waiting_input",
  error: "state.error",
  unknown: "state.unknown",
  idle: "state.idle",
  completed: "state.completed",
};

// CSS class per state (colors defined in styles.css).
export const STATE_CLASS: Record<InstanceState, string> = {
  working: "s-working",
  waiting_permission: "s-permission",
  waiting_input: "s-input",
  error: "s-error",
  unknown: "s-unknown",
  idle: "s-idle",
  completed: "s-completed",
};
