# 0010 — Cliente de escritorio egui con terminales embebidas

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

El cliente principal era una TUI lanzada desde la consola. El operador corrigió la
dirección: quiere una **aplicación instalable**, abierta desde el menú, orientada a agentes
y con terminales dentro de la propia ventana. El backend (daemon, orquestación, política,
gates, memoria, git) no cambia; cambia el cliente.

Restricciones reales:

- Linux primero, sin toolchain de Node en el build.
- Terminal real (PTY), no un panel de logs: hay que poder ejecutar `opencode`, `git` o los
  tests dentro de la app.
- Los ajustes se hacen desde la interfaz, no editando variables de entorno.
- El estado durable sigue en `jcsd`; cerrar la ventana no puede perder trabajo.

## Alternativas consideradas

1. **Tauri 2 + web (xterm.js).** UI moderna y portabilidad futura, pero introduce un
   frontend TypeScript, IPC entre dos runtimes y un build con Node. Para un producto de un
   solo desarrollador, dos lenguajes y dos sistemas de build es coste sin beneficio inmediato.
2. **GTK4 + libadwaita + VTE.** El terminal VTE es el de GNOME Terminal, maduro y nativo.
   A cambio: solo Linux, bindings y estilos propios, y Flatpak/deb como camino de
   distribución. La integración sería excelente, pero ata el producto a una plataforma y a
   una API de widgets con más ceremonia.
3. **egui/eframe puro.** Un lenguaje, un binario, control total del render y del estado. El
   coste es que el widget de terminal se construye y se mantiene; no hay DOM ni ecosistema
   de componentes.

## Decisión

- **Framework:** `eframe`/`egui` **0.31**, anclado a propósito. La 0.37 rediseñó
  `eframe::App` (`update` → `ui`) y movió los paneles; adoptar una API recién cambiada en la
  primera GUI es un riesgo innecesario. Actualizar es una tarea con migración y tests.
- **Terminal:** `portable-pty` para el PTY y `vt100` para el parser y la pantalla. No se usa
  `egui_term` porque está anclado a una versión vieja de egui. El widget dibuja la rejilla
  por celdas, traduce teclado (flechas, control, pegar) y redimensiona el PTY al área visible.
- **Cliente fino:** la app pide `/status` cada dos segundos y consume
  `GET /events?since=<seq>` con reconexión y cursor. Los comandos son los mismos endpoints
  HTTP/UDS que usa el CLI; no hay una segunda lógica de negocio.
- **Hilo de conexión:** un hilo dedicado con un runtime Tokio de un solo hilo. La UI nunca
  hace I/O ni espera: envía comandos por canal y drena actualizaciones una vez por frame.
  Las operaciones lentas (gates, indexación) corren en su propia tarea.
- **Ajustes:** tabla `settings` (una fila por clave), variables de entorno como valor
  **inicial**, validación en el daemon y evento de auditoría `settings_changed`. Un cambio
  que no puede aplicarse en caliente (`max_concurrent`) se marca como «requiere reiniciar».
  Los secretos no entran en SQLite: van al llavero del sistema (pendiente).
- **Empaquetado:** tarball con `jcsd`, `jcs`, `jc-studio`, unidad systemd, `.desktop`, icono
  y checksum. `install.sh` es no destructivo y reescribe `Exec` a la ruta real.
- **Alcance del terminal:** primero en el proceso de la app (sesiones vivas mientras la
  ventana está abierta). Mover el PTY al daemon para que sobreviva al cierre es el
  siguiente paso y no cambia el contrato del widget.

## Consecuencias

- **A favor:** un solo lenguaje y un solo binario de UI; sin Node; el mismo estado y los
  mismos endpoints que el CLI; el terminal y los ajustes son código propio y testeable.
- **En contra:** hay que mantener el emulador (scrollback, selección, portapapeles) y sus
  pruebas; la UI es inmediata, así que cada panel debe leer el snapshot con cuidado para no
  pedir de más; no hay inspección DOM para tests de interfaz.
- **Licencias:** las fuentes por defecto de egui son OFL-1.1 y Ubuntu-font-1.0; se permiten
  explícitamente en `deny.toml` y viajan con el paquete de la fuente.
- **Distribución:** el tarball es honesto pero no es un `.deb`/AppImage; se documenta como
  pendiente y el `.desktop` ya deja la app en el menú.

## Verificación

- `cargo test -p jcs-app`: formularios, estado, deduplicación de eventos y **un PTY real**
  que ejecuta un comando y llega a la pantalla (`a_command_runs_and_reaches_the_screen`).
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `./scripts/package.sh` + `desktop-file-validate contrib/jc-studio.desktop` + instalación
  en un prefijo limpio con `JCS_BIN_DIR` y `XDG_DATA_HOME` temporales.
- Prueba manual: abrir la app con un daemon vivo, ver el proyecto y el objetivo, y ejecutar
  un comando en la terminal embebida.
