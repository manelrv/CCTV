import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import type { Instance } from "../types";
import { InstanceRow } from "./InstanceRow";

/** Resolves the effective dark/light palette given a theme preference and the OS setting. */
function resolveTheme(theme: string, prefersDark: boolean): "dark" | "light" {
  if (theme === "dark") return "dark";
  if (theme === "light") return "light";
  return prefersDark ? "dark" : "light";
}

// Auto-resize height bounds: enough for titlebar+summary+placeholder at min,
// capped at max so rows beyond this point scroll instead of growing forever.
const MIN_HEIGHT = 120;
const MAX_HEIGHT = 600;

export function MonitorWindow({
  instances,
  now,
  compact,
  opacity = 92,
  theme = "system",
}: {
  instances: Instance[];
  now: number;
  compact: boolean;
  opacity?: number;
  theme?: string;
}) {
  const { t } = useTranslation();
  const attention = instances.filter(
    (i) => i.state === "waiting_permission" || i.state === "waiting_input"
  ).length;

  const panelRef = useRef<HTMLDivElement>(null);
  // Track last height sent to avoid feedback-loop storms:
  //   ResizeObserver fires → setSize → OS resizes window → observer fires again.
  // We only call setSize when the clamped target differs from the last sent value.
  const lastSentHeight = useRef<number>(0);

  // Apply opacity and theme to the root element whenever they change.
  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty("--panel-opacity", String(opacity / 100));

    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const applyTheme = () => {
      const resolved = resolveTheme(theme, mq.matches);
      root.setAttribute("data-theme", resolved);
    };

    applyTheme();

    // Listen for OS theme changes when preference is "system".
    if (theme === "system") {
      mq.addEventListener("change", applyTheme);
      return () => mq.removeEventListener("change", applyTheme);
    }
    return undefined;
  }, [opacity, theme]);

  useEffect(() => {
    const el = panelRef.current;
    if (!el) return;

    let rafId: number | undefined;

    const applyHeight = () => {
      // The panel fills the window (height:100%), so panel.scrollHeight can't be
      // used to shrink. Measure the LIST's natural content height (scrollHeight,
      // which ignores the visible clip) plus the fixed chrome (titlebar + summary
      // + borders) = panel height minus the list's visible height.
      const list = el.querySelector<HTMLElement>(".list");
      const listNatural = list ? list.scrollHeight : 0;
      const chrome = el.clientHeight - (list ? list.clientHeight : 0);
      const natural = chrome + listNatural;
      const target = Math.min(Math.max(natural, MIN_HEIGHT), MAX_HEIGHT);

      if (target !== lastSentHeight.current) {
        lastSentHeight.current = target;
        // Use the native resize command, NOT window.setSize(): the latter is a
        // no-op on our frameless (decorations:false) macOS window (tauri#11975).
        invoke("resize_monitor", { height: target }).catch((err) => {
          console.warn("[auto-resize] resize_monitor failed:", err);
        });
      }
    };

    // Debounce via rAF: if multiple mutations arrive in the same frame we
    // only call setSize once, preventing micro-storm bursts.
    const observer = new ResizeObserver(() => {
      if (rafId !== undefined) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        rafId = undefined;
        applyHeight();
      });
    });

    // Also run immediately on mount / whenever instances or compact change.
    applyHeight();

    observer.observe(el);
    return () => {
      observer.disconnect();
      if (rafId !== undefined) cancelAnimationFrame(rafId);
    };
    // Re-run the effect when the list or compact flag change so we re-measure
    // even if the ResizeObserver hasn't fired yet (e.g. very fast renders).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instances, compact]);

  return (
    // The "compact" class activates the compact-mode CSS rules (styles.css).
    <div ref={panelRef} className={compact ? "panel compact" : "panel"}>
      {/* Title bar: draggable region for moving the frameless window */}
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
