import { useTranslation } from "react-i18next";
import type { Instance } from "../types";
import { STATE_CLASS, STATE_I18N_KEY, formatTokens } from "../types";

function timeInState(lastEventAt: number, now: number): string {
  const secs = Math.max(0, now - lastEventAt);
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function InstanceRow({ inst, now }: { inst: Instance; now: number }) {
  const { t } = useTranslation();
  const cls = STATE_CLASS[inst.state];
  return (
    <div className={`row ${cls}`}>
      <span className="dot" aria-hidden />
      <div className="row-main">
        <div className="project">
          {inst.project}
          <span className="source-badge">{inst.source === "background" ? "bg" : "fg"}</span>
        </div>
        {inst.detail && <div className="detail">{inst.detail}</div>}
      </div>
      <div className="row-meta">
        <div className="state">{t(STATE_I18N_KEY[inst.state])}</div>
        <div className="time-tokens">
          <span className="time">{timeInState(inst.last_event_at, now)}</span>
          {inst.context_tokens != null && (
            <span className="ctx-tokens">{formatTokens(inst.context_tokens)}</span>
          )}
        </div>
      </div>
    </div>
  );
}
