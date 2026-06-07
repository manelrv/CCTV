# CCTV

**C**laude **C**ode **T**ele**v**isió — ventana flotante *always-on-top* +
icono de bandeja que muestra el estado de **todas las instancias de Claude
Code** corriendo en la máquina: si están trabajando, esperando que apruebes
algo, esperando input, terminadas o con error.

![estados](docs/states.png)
<!-- TODO(claude-code): añadir captura actualizada de la UI -->

## Cómo funciona

El estado llega de **dos fuentes** que se fusionan en un único store:

- **Segundo plano** (`claude --bg`, Agent View): un *file watcher* lee los
  `state.json` que el supervisor de Claude Code persiste en `~/.claude/jobs/`.
- **Primer plano** (sesiones de terminal): *hooks* HTTP que hacen POST a un
  servidor local que vive dentro de esta app.

Detalle completo en [`CLAUDE.md`](CLAUDE.md), [`docs/DATA-SOURCES.md`](docs/DATA-SOURCES.md)
y [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Requisitos

- Node.js 18+
- Rust estable + toolchain de Tauri 2 (https://v2.tauri.app/start/prerequisites/)

## Arranque (dev)

```bash
npm install
npm run tauri dev
```

## Conectar los hooks (primer plano)

Fusiona la clave `hooks` de [`hooks/settings.snippet.json`](hooks/settings.snippet.json)
dentro de tu `~/.claude/settings.json`. Dentro de Claude Code, `/hooks` debe
listarlos con fuente `User`. A partir de ahí, cada sesión que abras aparece en
la ventana. Las sesiones en segundo plano no necesitan configuración.

## Estado del proyecto

Funcional en macOS: fuente híbrida, máquina de estados verificada con sesiones
reales, reaper TTL, bandeja con icono dinámico y preferencias, i18n (8 idiomas)
y float sobre apps en fullscreen (NSPanel). Pendiente: Linux/Wayland, Windows y
empaquetado. Historial en [`WORKLOG.md`](WORKLOG.md) y estado por fases en
[`docs/ROADMAP.md`](docs/ROADMAP.md).

## Plataformas

macOS (principal) → Linux → Windows. Notas por plataforma, incluida la regla de
Hyprland para Wayland, en [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
