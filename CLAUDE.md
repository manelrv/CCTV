# CLAUDE.md

Contexto del proyecto para Claude Code. Léelo entero antes de tocar nada.

## Qué es esto

Una app de escritorio que **monitoriza todas las instancias de Claude Code que
corren en la máquina** y muestra, en una ventana flotante *always-on-top* (más un
icono en la bandeja del sistema), el estado de cada una: si está trabajando, si
está esperando que el usuario apruebe algo, si espera input, o si ha terminado.

El objetivo es no tener que ir mirando terminal por terminal para saber qué
agente te reclama.

## Arquitectura en una frase

El estado de las instancias llega de **dos fuentes**: sesiones en segundo plano
se leen desde `~/.claude/jobs/` (file watcher), y sesiones en primer plano llegan
por **hooks HTTP** desde `~/.claude/settings.json`. Ambas se fusionan en un único
store que empuja snapshots a la ventana (webview) por eventos de Tauri.

```
Claude Code — sesiones bg          Claude Code — sesiones fg
  ~/.claude/jobs/<id>/state.json        hooks HTTP (POST localhost:8787)
        │  file watcher                       │
        ▼                                     ▼
App Tauri (proceso siempre vivo en la bandeja)
  ├── jobs.rs    → Fuente A: watcher + parse state.json (notify + dirs)
  ├── server.rs  → Fuente B: recibe los hooks (axum)
  ├── state.rs   → store híbrido + merge rule "background manda" + reaper TTL
  ├── tray.rs    → icono + menú de preferencias
  └── webview    → ventana flotante React (recibe snapshots por evento)
```

Ver `docs/ARCHITECTURE.md` para el detalle, `docs/HOOKS.md` para el esquema de
payloads y `docs/DATA-SOURCES.md` para la regla de fusión de fuentes.

## Stack

- **Tauri 2** (Rust + webview). Elegido por footprint mínimo (la app está siempre
  corriendo) y por soporte nativo de ventana sin marco, transparente,
  always-on-top y bandeja. Si en algún momento se decide migrar a Electron, el
  frontend React se reaprovecha tal cual.
- **Backend:** Rust con `axum` (servidor HTTP de los hooks) + `tokio`.
- **Frontend:** React + TypeScript + Vite. CSS plano (sin framework) en
  `src/styles.css`.

## Plataformas (en orden de prioridad)

1. **macOS** — objetivo principal. Funciona todo directo.
2. **Linux** — X11 directo. **Wayland: el always-on-top depende del compositor.**
   Para Hyprland se resuelve con reglas del compositor, no con la API de ventana.
   Ver `docs/ARCHITECTURE.md#linux--wayland`.
3. **Windows** — sin fricción.

## Idiomas de la app

La app debe ser **multilingüe**. Estos son los idiomas que deben estar presentes:

1. Inglés
2. Español
3. Portugués
4. Alemán
5. Francés
6. Italiano
7. Catalán
8. Ruso

Idiomo por defecto:

1. Inglés

## Arquitectura de la app

El documento `docs/ARCHITECTURE.md` describe la arquitectura de la app. Mantenlo actualizado.

## Reglas de trabajo

- Mantén `docs/ROADMAP.md` actualizado: marca lo hecho, añade lo que descubras.
- El esquema de los hooks es la fuente de verdad: si algo no encaja con
  `docs/HOOKS.md`, **verifica contra la doc oficial** antes de improvisar tipos.
  Doc: https://code.claude.com/docs/en/hooks
- No bloquees nunca a Claude Code: el endpoint HTTP debe responder `200` con
  cuerpo vacío de inmediato. Toda la lógica va después de responder, o en otra
  tarea. Un hook lento ralentiza la sesión del usuario.
- Los `TODO(claude-code):` repartidos por el código marcan lo que falta por
  implementar. Búscalos con grep.

## Cómo arrancar (dev)

```bash
npm install
npm run tauri dev      # levanta Vite + compila Rust + abre la app
```

Para que los hooks lleguen, hay que tener la config de `hooks/settings.snippet.json`
fusionada en `~/.claude/settings.json` (ver `docs/HOOKS.md#instalación`).

## Estado actual

Scaffold inicial. El núcleo (tipos de hooks, rutas del servidor, máquina de
estados, reaper TTL, bandeja básica, ventana React con la UI) está esbozado y
es coherente, pero **no se ha compilado ni probado todavía**. Primera tarea:
hacer que `npm run tauri dev` compile y arranque. Ver `docs/ROADMAP.md` fase 0.
