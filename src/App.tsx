import { useEffect, useState } from "react";
import { MonitorWindow } from "./components/MonitorWindow";
import { fetchInstances, fetchPrefs, onInstances, onPrefs } from "./lib/ipc";
import { applyLanguagePref } from "./i18n";
import type { Instance, Prefs } from "./types";

const DEFAULT_PREFS: Prefs = {
  floating_window: true,
  always_on_top: true,
  auto_hide: false,
  compact: false,
  open_at_login: true,
  opacity: 92,
  theme: "system",
  language: "auto",
};

export default function App() {
  const [instances, setInstances] = useState<Instance[]>([]);
  const [prefs, setPrefs] = useState<Prefs>(DEFAULT_PREFS);
  // "now" in seconds, refreshed every second for time counters.
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    // Apply prefs to state and switch the UI language to match the preference.
    const applyPrefs = (p: Prefs) => {
      applyLanguagePref(p.language);
      setPrefs(p);
    };

    fetchInstances().then(setInstances);
    fetchPrefs().then(applyPrefs);

    const unlistenInstances = onInstances(setInstances);
    const unlistenPrefs = onPrefs(applyPrefs);

    return () => {
      unlistenInstances.then((fn) => fn());
      unlistenPrefs.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const id = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <MonitorWindow
      instances={instances}
      now={now}
      compact={prefs.compact}
      opacity={prefs.opacity}
      theme={prefs.theme}
    />
  );
}
