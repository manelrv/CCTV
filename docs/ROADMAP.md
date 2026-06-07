# Roadmap

Marca `[x]` lo hecho. Cada fase debería dejar la app en un estado arrancable.

## Fase 0 — Compila y arranca ✅
- [x] `npm install` y `npm run tauri dev` compilan y abren una ventana.
      (Arreglos: `[lib]` espurio en Cargo.toml, feature `macos-private-api`,
      `.manage(store)` en main.rs, `server.rs` duplicado en la raíz eliminado.)
- [x] Generar iconos (`npx tauri icon icons/icon-app-1024.png`). Fuentes en
      `icons/` (app + tray calm/alert para fase 4).
- [x] La ventana muestra datos *mock* con la UI de filas. Sembrados en el store
      (`main.rs`, solo `debug_assertions`) — prueban el pipeline completo.
      Quitar en fase 1 (marcado con TODO).

## Fase 1 — Recibir hooks de verdad ✅
- [x] Servidor axum escuchando en `127.0.0.1:8787` (`/health` responde OK).
- [x] Las 8 rutas de `docs/HOOKS.md` parsean el payload y responden `200` vacío.
      (Las de `Notification` quedan por ejercitar con sesiones reales — fase 2.)
- [x] Fusionar `hooks/settings.snippet.json` en `~/.claude/settings.json`.
      Backup en `settings.json.bak-pre-cctv-hooks`; el hook previo de
      `UserPromptSubmit` (gentle-ai) convive en el mismo array.
- [x] Sesión real de Claude Code aparece en la ventana y cambia de estado.
      Mocks de fase 0 eliminados de `main.rs`.

## Fase 1b — Fuente híbrida (Agent View)
- [x] Watcher de `~/.claude/jobs/` integrado (`jobs.rs`, crates `notify` + `dirs`).
- [x] Esquema de `state.json` verificado empíricamente (sessionId camelCase,
      detail/intent, createdAt/updatedAt RFC3339; campo `name` también presente).
- [x] Regla "background manda" implementada en `set_background_snapshot()`.
- [x] Reaper TTL acotado solo a instancias Foreground.
- [x] Estados `Completed` y `Error` añadidos a `InstanceState` con urgency.
- [x] Badge `bg`/`fg` discreto en cada fila de la UI.
- [x] Traducciones de `state.completed` y `state.error` en los 8 idiomas.
- [x] Ejercitar estados de background no observados con sesiones reales.
      Hallazgo: `state` solo no distingue permiso de input — la clave es la
      combinación `state`+`tempo` (working+blocked → permiso; blocked+blocked
      → input). `map_state` ajustado; campo `needs` usado como detalle.

## Fase 2 — Máquina de estados + UI en vivo ✅
- [x] Transiciones de `state.rs` completas y probadas con sesiones reales
      (fg vía hooks; bg vía experimentos con `claude --bg` + `claude stop`).
- [x] `emit("instances", ...)` y el frontend pinta cambios en vivo.
- [x] Orden por urgencia (permiso > input > error > trabajando > unknown >
      idle > completado).
- [x] Derivar nombre de proyecto desde `cwd`: `$HOME` → `~`, abreviado a los
      2 últimos segmentos si es profundo. Con tests unitarios (`cargo test`).
- [x] Resumen del detalle de tool (`tool_name` + corte de `tool_input`),
      verificado en vivo ("Bash · git ls-remote …").

## Fase 3 — Sesiones muertas ✅
- [x] Reaper TTL: `Working` viejo → `Unknown`; muy viejo → eliminar. Cubierto
      con 7 tests unitarios (TTL, scope foreground-only, regla de fusión).
- [x] Probado matando una sesión a lo bruto (`kill -9`, sin `SessionEnd`):
      `working` → `unknown` verificado en vivo a los ~230s vía
      `GET /debug/snapshot` (endpoint nuevo de introspección, solo loopback).
- Nota: el store es memoria pura — reiniciar la app borra las instancias fg
      hasta que sus sesiones emitan el siguiente hook. Es el comportamiento
      esperado, no un bug.

## Fase 4 — Bandeja y preferencias ✅
- [x] Icono refleja el estado: calm (tray-calm-64.png) cuando attention_count==0,
      alert (tray-alert-64.png) cuando >0. Titulo numerico en macOS junto al icono.
- [x] Propagacion centralizada en `refresh.rs::refresh()`: sustituye los tres
      puntos de emision dispersos (server.rs, jobs.rs, reaper de main.rs).
- [x] Toggles del menú cableados:
      - `floating_window`: show/hide ventana (ya funcionaba, verificado).
      - `always_on_top`: set_always_on_top (ya funcionaba, verificado).
      - `auto_hide`: oculta ventana cuando attention==0; muestra cuando >0
        (solo si floating_window está activo). Aplica de inmediato al toggle.
      - `compact`: emite evento "prefs" al frontend; aplica clase CSS `.compact`
        (oculta `.detail`, reduce padding de fila).
      - `open_at_login`: usa `tauri-plugin-autostart` (enable/disable via
        `ManagerExt::autolaunch()`).
- [x] `PrefsState` como managed state (`Mutex<Prefs>`): refresh() lee prefs sin
      I/O de disco en cada evento de hook.
- [x] `config.rs`: añadidos `load_from_path()` y `default_prefs_path()` para
      inicializar el managed state antes del setup().
- [x] Frontend: `Prefs` type en types.ts, `onPrefs`/`fetchPrefs` en ipc.ts,
      prop `compact` en MonitorWindow, clase `.compact` en styles.css.
- [x] 4 tests nuevos en `refresh.rs` (tray_variant + Prefs serde). Total: 15.

## Fase 5 — Pulido por plataforma
- [x] macOS: flotar sobre fullscreen. Integración de `tauri-nspanel` (branch
      `v2.1`, commit `a3122e89`). `macos.rs` convierte el `WebviewWindow` en un
      `NSPanel` subclass vía `to_panel::<MonitorPanel>()` y lo configura con:
      - `StyleMask::empty().nonactivating_panel()` — no roba foco.
      - `PanelLevel::Status` (25) — penetra el Space de fullscreen.
      - `CollectionBehavior::can_join_all_spaces + full_screen_auxiliary + stationary`.
      El enfoque NSWindow+FullScreenAuxiliary+level 101 fue descartado tras
      verificación empírica: la ventana desaparece igualmente al entrar en
      fullscreen aunque los bits estén correctamente aplicados (confirmado por
      logs). NSPanel es requerido por macOS para esta garantía.
      Verificación manual pendiente (requiere GUI fullscreen).
- [ ] Linux/Wayland: regla de Hyprland documentada y `class` real verificada.
- [ ] Windows: build y prueba.
- [ ] Empaquetado: `.dmg` / `.AppImage`+`.deb` / `.msi`.

## Ideas / backlog
- Click en una fila → traer al frente esa terminal (difícil multiplataforma) o
  copiar el `cwd`.
- Histórico de tiempos por sesión (SQLite) para métricas.
- Notificación de escritorio al pasar a `WaitingPermission` (vía el propio
  `terminalSequence` del hook, o nativa de la app).
- Subagentes: `SubagentStart`/`SubagentStop` como sub-filas anidadas.
