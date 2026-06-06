import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Instance } from "../types";

/** Suscribe a los snapshots que empuja el backend. Devuelve el unlisten. */
export function onInstances(cb: (rows: Instance[]) => void) {
  return listen<Instance[]>("instances", (e) => cb(e.payload));
}

/** Pide el snapshot inicial al montar (por si la app ya tenia estado). */
export async function fetchInstances(): Promise<Instance[]> {
  try {
    return await invoke<Instance[]>("get_instances");
  } catch {
    // get_instances aun no registrado (ver NOTA en main.rs) -> vacio.
    return [];
  }
}
