/** Espejo TS de config::Prefs en Rust. Serializado por serde con snake_case. */
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
}

// Claves de traducción para cada estado (resueltas via t() en el componente).
export const STATE_I18N_KEY: Record<InstanceState, string> = {
  working: "state.working",
  waiting_permission: "state.waiting_permission",
  waiting_input: "state.waiting_input",
  error: "state.error",
  unknown: "state.unknown",
  idle: "state.idle",
  completed: "state.completed",
};

// Clase CSS por estado (los colores viven en styles.css).
export const STATE_CLASS: Record<InstanceState, string> = {
  working: "s-working",
  waiting_permission: "s-permission",
  waiting_input: "s-input",
  error: "s-error",
  unknown: "s-unknown",
  idle: "s-idle",
  completed: "s-completed",
};
