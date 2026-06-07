import { useEffect, useState } from "react";
import { MonitorWindow } from "./components/MonitorWindow";
import { fetchInstances, fetchPrefs, onInstances, onPrefs } from "./lib/ipc";
import type { Instance, Prefs } from "./types";

const DEFAULT_PREFS: Prefs = {
  floating_window: true,
  always_on_top: true,
  auto_hide: false,
  compact: false,
  open_at_login: true,
};

export default function App() {
  const [instances, setInstances] = useState<Instance[]>([]);
  const [prefs, setPrefs] = useState<Prefs>(DEFAULT_PREFS);
  // "now" en segundos, refrescado cada segundo para los contadores de tiempo.
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    fetchInstances().then(setInstances);
    fetchPrefs().then(setPrefs);

    const unlistenInstances = onInstances(setInstances);
    const unlistenPrefs = onPrefs(setPrefs);

    return () => {
      unlistenInstances.then((fn) => fn());
      unlistenPrefs.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const id = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(id);
  }, []);

  return <MonitorWindow instances={instances} now={now} compact={prefs.compact} />;
}
