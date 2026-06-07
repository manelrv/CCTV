import { useTranslation } from "react-i18next";
import type { Instance } from "../types";
import { InstanceRow } from "./InstanceRow";

export function MonitorWindow({
  instances,
  now,
  compact,
}: {
  instances: Instance[];
  now: number;
  compact: boolean;
}) {
  const { t } = useTranslation();
  const attention = instances.filter(
    (i) => i.state === "waiting_permission" || i.state === "waiting_input"
  ).length;

  return (
    // La clase "compact" activa las reglas CSS de modo compacto (styles.css).
    <div className={compact ? "panel compact" : "panel"}>
      {/* Barra de titulo: zona arrastrable para mover la ventana sin marco */}
      <div className="titlebar" data-tauri-drag-region>
        <span className="title">{t("title")}</span>
      </div>

      <div className="summary">
        <span>{t("summary.instances", { count: instances.length })}</span>
        {attention > 0 && (
          <span className="attention">
            {t("summary.attention", { count: attention })}
          </span>
        )}
      </div>

      <div className="list">
        {instances.length === 0 ? (
          <div className="empty">{t("empty")}</div>
        ) : (
          instances.map((inst) => (
            <InstanceRow key={inst.session_id} inst={inst} now={now} />
          ))
        )}
      </div>
    </div>
  );
}
