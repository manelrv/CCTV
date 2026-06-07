import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Instance, Prefs } from "../types";

/** Suscribe a los snapshots que empuja el backend. Devuelve el unlisten. */
export function onInstances(cb: (rows: Instance[]) => void) {
  return listen<Instance[]>("instances", (e) => cb(e.payload));
}

/** Pide el snapshot inicial al montar (por si la app ya tenia estado). */
export async function fetchInstances(): Promise<Instance[]> {
  try {
    return await invoke<Instance[]>("get_instances");
  } catch {
    return [];
  }
}

/** Suscribe a los cambios de preferencias empujados por el backend. */
export function onPrefs(cb: (prefs: Prefs) => void) {
  return listen<Prefs>("prefs", (e) => cb(e.payload));
}

/** Pide las preferencias actuales al montar. */
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
    };
  }
}
