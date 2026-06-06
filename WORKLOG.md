# WORKLOG — CCTV

Registro cronológico del trabajo realizado. Formato: fecha + fase + bullets concisos.

---

## 2026-06-06

### Fase 0 — Compilación y arranque inicial

- Arreglos para que `npm run tauri dev` compilara:
  - Eliminada sección `[lib]` espuria en `Cargo.toml`.
  - Añadida feature `macos-private-api` a la dependencia de tauri.
  - Añadido `.manage(store)` en `main.rs` (faltaba para el comando `get_instances`).
  - Eliminado `server.rs` duplicado que había en la raíz del proyecto.
- Generados iconos con `npx tauri icon icons/icon-app-1024.png`.
- Ventana muestra datos mock (sembrados en store bajo `debug_assertions`) — pipeline completo funcionando.

### Fase 1 — Hooks reales

- Servidor axum escuchando en `127.0.0.1:8787`; `/health` responde OK.
- Las 8 rutas de `docs/HOOKS.md` parsean payload y responden `200` vacío inmediato.
- Hooks fusionados en `~/.claude/settings.json` con backup previo (`settings.json.bak-pre-cctv-hooks`).
- Hook previo de `UserPromptSubmit` (gentle-ai) convive en el mismo array sin conflicto.
- Sesión real de Claude Code verificada: aparece en la ventana y cambia de estado.
- Mocks de fase 0 eliminados de `main.rs`.

### i18n + renombre

- App renombrada de "Claude Code Monitor" a "CCTV" (nombre visible en la UI).
- 8 idiomas implementados: en, es, pt, de, fr, it, ca, ru.
- Estructura de claves: `state.*`, `summary.*`, `empty`.
- Russo tiene 3 formas de plural correctamente configuradas.

### Fase 1b — Fuente híbrida (Agent View)

- Esquema real de `~/.claude/jobs/<id>/state.json` verificado empíricamente con
  sesión `be4c186b`. Campos clave: `sessionId` (camelCase), `state`, `detail`,
  `intent`, `name`, `cwd`, `createdAt`, `updatedAt` (RFC3339), `daemonShort`.
  Los campos `status`, `summary`, `title` que asumía el scaffold NO existen.
- `jobs.rs` corregido: struct `JobState` con `serde(rename_all = "camelCase")`;
  campos defensivos `Option`; detalle con fallback `detail → intent → name`.
- Timestamps RFC3339 → epoch secs implementado con parser manual (sin chrono).
  Fallback a mtime del fichero cuando falta o es inparseable.
- `state.rs` extendido:
  - Enum `Source { Background, Foreground }` añadido y serializado.
  - `Instance` gana campo `source: Source`.
  - `InstanceState` gana `Completed` y `Error` con urgency correcto.
  - `apply()` marca `source: Foreground` en insert (path de hooks).
  - `set_background_snapshot()`: elimina background anterior + foreground solapado, inserta nuevo set.
  - `reap()`: solo toca instancias Foreground.
  - `project_from_cwd` pasa a `pub(crate)` (importado desde `jobs.rs`).
- `main.rs`: añadido `mod jobs` y llamada `jobs::start(store, handle)` tras el spawn del servidor.
- `Cargo.toml`: añadidos `notify = "6"` y `dirs = "5"`.
- Frontend:
  - `types.ts`: `InstanceState` gana `"completed"` y `"error"`; `Instance` gana `source: Source`.
  - `InstanceRow.tsx`: badge `bg`/`fg` junto al nombre de proyecto.
  - `styles.css`: `.s-completed` (verde suave) y `.s-error` (rojo) + `.source-badge`.
  - 8 ficheros i18n: añadidos `state.completed` y `state.error`.
- Docs: `ARCHITECTURE.md` actualizado con sección de fuentes híbridas y módulo `jobs.rs`;
  `CLAUDE.md` diagrama ASCII actualizado para mostrar las dos fuentes;
  `ROADMAP.md` sección Fase 1b añadida.

---

_Verificación final: `cargo check` 0 errores · `tsc --noEmit` 0 errores · `npm run build` clean._
