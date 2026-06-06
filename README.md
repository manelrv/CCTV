# Claude Code Monitor

Ventana flotante *always-on-top* + icono de bandeja que muestra el estado de
**todas las instancias de Claude Code** corriendo en la máquina: si están
trabajando, esperando que apruebes algo, esperando input, o terminadas.

![estados](docs/states.png)
<!-- TODO(claude-code): añadir captura cuando la UI funcione -->

## Cómo funciona

Claude Code dispara *hooks* HTTP que hacen POST a un servidor local que vive
dentro de esta app. La app mantiene una máquina de estados por sesión y la pinta
en la ventana. Detalle completo en [`CLAUDE.md`](CLAUDE.md) y
[`docs/`](docs/).

## Requisitos

- Node.js 18+
- Rust estable + toolchain de Tauri 2 (https://v2.tauri.app/start/prerequisites/)

## Arranque (dev)

```bash
npm install

# Iconos (necesarios para compilar). Genera desde un PNG cuadrado:
npm run tauri icon ruta/a/un-icono-1024.png

npm run tauri dev
```

## Conectar los hooks

Fusiona la clave `hooks` de [`hooks/settings.snippet.json`](hooks/settings.snippet.json)
dentro de tu `~/.claude/settings.json`. Dentro de Claude Code, `/hooks` debe
listarlos con fuente `User`. A partir de ahí, cada sesión que abras aparece en
la ventana.

## Estado del proyecto

Scaffold inicial pensado para que **Claude Code continúe el desarrollo**.
El núcleo (tipos de hooks, servidor, máquina de estados, reaper, bandeja, UI)
está esbozado pero **sin compilar todavía**. Empieza por
[`docs/ROADMAP.md`](docs/ROADMAP.md) fase 0.

## Plataformas

macOS (principal) → Linux → Windows. Notas por plataforma, incluida la regla de
Hyprland para Wayland, en [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
