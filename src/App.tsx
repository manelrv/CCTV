import { useEffect, useState } from "react";
import { MonitorWindow } from "./components/MonitorWindow";
import { fetchInstances, onInstances } from "./lib/ipc";
import type { Instance } from "./types";

export default function App() {
  const [instances, setInstances] = useState<Instance[]>([]);
  // "now" en segundos, refrescado cada segundo para los contadores de tiempo.
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  useEffect(() => {
    fetchInstances().then(setInstances);
    const unlisten = onInstances(setInstances);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const id = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(id);
  }, []);

  return <MonitorWindow instances={instances} now={now} />;
}
