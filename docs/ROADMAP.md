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
- [ ] Ejercitar estados de background no observados aún (`needs_input`, `failed`,
      `stopped`) con sesiones reales para confirmar el mapeo.

## Fase 2 — Máquina de estados + UI en vivo
- [ ] Transiciones de `state.rs` completas y probadas con sesiones reales.
- [ ] `emit("instances", ...)` y el frontend pinta cambios en vivo.
- [ ] Orden por urgencia (permiso > input > trabajando > idle/unknown).
- [ ] Derivar nombre de proyecto desde `cwd` (último segmento o `~/...`).
- [ ] Resumen del detalle de tool (`tool_name` + corte de `tool_input`).

## Fase 3 — Sesiones muertas
- [ ] Reaper TTL: `Working` viejo → `Unknown`; muy viejo → eliminar.
- [ ] Probar matando una sesión a lo bruto (sin `SessionEnd`).

## Fase 4 — Bandeja y preferencias
- [ ] Icono refleja el estado más urgente (color/contador).
- [ ] Toggles del menú cableados a su efecto.
- [ ] Persistencia de prefs en `config.rs`.
- [ ] `auto_hide`: ocultar cuando nada reclama, reaparecer ante permiso/idle.
- [ ] `open_at_login` (autostart) — usar plugin `tauri-plugin-autostart`.

## Fase 5 — Pulido por plataforma
- [ ] macOS: flotar sobre fullscreen (NSWindow level si hace falta).
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
