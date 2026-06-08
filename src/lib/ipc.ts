import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Instance, Prefs } from "../types";

/** Subscribes to snapshots pushed by the backend. Returns the unlisten function. */
export function onInstances(cb: (rows: Instance[]) => void) {
  return listen<Instance[]>("instances", (e) => cb(e.payload));
}

/** Fetches the initial snapshot on mount (in case the app already had state). */
export async function fetchInstances(): Promise<Instance[]> {
  try {
    return await invoke<Instance[]>("get_instances");
  } catch {
    return [];
  }
}

/** Subscribes to preference changes pushed by the backend. */
export function onPrefs(cb: (prefs: Prefs) => void) {
  return listen<Prefs>("prefs", (e) => cb(e.payload));
}

/** Fetches the current preferences on mount. */
export async function fetchPrefs(): Promise<Prefs> {
  try {
    return await invoke<Prefs>("get_prefs");
  } catch {
    return {
      floating_window: true,
      always_on_top: true,
      auto_hide: false,
      compact: false,
      open_at_login: true,
      opacity: 92,
      theme: "system",
    };
  }
}

