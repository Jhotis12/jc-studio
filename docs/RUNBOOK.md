# Runbook

Operación de JC Studio en una máquina de trabajo. Para el estado del proyecto y lo que
falta, ver [`STATUS.md`](STATUS.md); para retomar el desarrollo, [`HANDOFF.md`](HANDOFF.md).

## Requisitos

- Linux o macOS, `git` ≥ 2.30, Rust 1.89+.
- OpenCode con su servicio en marcha (`opencode service status`); el daemon se conecta a él.
- Opcional: Ollama con un modelo de embeddings para búsqueda semántica.
- Opcional: un bot de Telegram para operar desde el móvil.

## Instalación

**Tarball (sin Rust):**

```bash
tar -xzf jc-studio-<versión>-x86_64-unknown-linux-gnu.tar.gz
cd jc-studio-<versión>-x86_64-unknown-linux-gnu
./install.sh
```

**Desde el repositorio:** `cargo build --release && ./contrib/install.sh`.

Ambos instalan en `~/.local/bin`, la unidad en `~/.config/systemd/user/`, y registran
**JC Studio** en el menú de aplicaciones (`.desktop` + icono).

```bash
systemctl --user daemon-reload
systemctl --user enable --now jcsd
journalctl --user -u jcsd -f
```

Sin systemd: `jcsd` en primer plano (los logs van a stderr).

## Comprobación

```bash
jcs doctor          # socket 0600, protocolo, backend, base de datos
jcs ping            # el daemon responde
jcs status          # proyectos, objetivos, tareas, runs y coste
jcs metrics         # texto Prometheus
```

## La app de escritorio

Abre **JC Studio** desde el menú o ejecuta `jc-studio`. Se conecta al mismo socket que el
CLI y reintenta cada dos segundos si el daemon no está.

| Zona | Qué hace |
|---|---|
| Proyectos | Registrar repositorios, ver objetivos y coste |
| Objetivos | Componer un objetivo, planificar en DAG, seguir el estado |
| Detalle | Tareas, dependencias, ejecuciones, gates y reanudación |
| Política | Packs, reglas, gates, drift y ejecución manual |
| Memoria | Estadísticas, indexación y búsqueda con citas |
| Código | Árbol de archivos, editor, archivos nuevos y diff del worktree |
| Sistema | Salud del daemon, rutas y totales |
| Consola | Eventos, traza del agente y terminales reales (PTY) |

Los **ajustes** se editan en la ventana «Ajustes»: modelos (con un selector que muestra el
precio de cada uno), agente, revisión, techo de riesgo, timeout, contexto, reparación y
concurrencia; por proyecto, auto-merge y presupuestos.

**Telegram** también se configura ahí: token (write-only), chats permitidos y estado
«configurado · …abcd». El token se guarda en `~/.local/share/jcs/secrets.json` con permisos
0600, no en SQLite; la API nunca lo devuelve. Se aplica al reiniciar `jcsd` (botón
«Reiniciar jcsd»). Sin chats permitidos el bot no responde a nadie. El daemon valida, guarda en la base y registra `settings_changed`. Las
variables de entorno son el **valor inicial**; a partir de ahí manda lo guardado. Cambiar
la concurrencia requiere reiniciar `jcsd` (la app lo avisa).

### Varios objetivos a la vez

El daemon reparte la capacidad entre los objetivos activos
(`max_concurrent / objetivos`, mínimo 1) y rota el orden para que ninguno acapare. Un
objetivo con tareas en vuelo puede seguir; el techo de presupuesto solo impide despachar
trabajo nuevo.

Las tareas con **claims de rutas solapados** no corren juntas: la app muestra «espera: …
trabaja en las mismas rutas». Las que no declaran rutas se consideran en conflicto con todo
lo que esté corriendo (conservador). Si dos objetivos tocan lo mismo, el segundo espera o,
al promocionar, se rebasa y se re-verifica; un conflicto bloquea el objetivo, no lo falla.

Para subir la concurrencia: **Ajustes → Concurrencia máxima → Guardar → Reiniciar jcsd**.

### Progreso y objetivos atascados

El progreso por tarea se muestra por **etapas** (en cola, agente trabajando, verificando,
integrando, completada, merge bloqueado, falló, bloqueada), no por porcentaje: no existe un
porcentaje real mientras el modelo piensa. El objetivo muestra una tira por tarea y los
conteos. Si no hay tareas en vuelo, ninguna puede empezar y no hay merges pendientes, el
objetivo pasa a `blocked`/`failed` con `goal_stalled`; ya no se queda `running` para siempre.

### Iterar una tarea

En el detalle de una tarea:

- **Pedir cambios** (tarea completada): crea una tarea nueva que depende de ella y lleva tus
  indicaciones; el trabajo original no se toca.
- **Reintentar con indicaciones** (fallida/bloqueada): vuelve a la cola con las instrucciones
  nuevas y reutiliza su worktree.
- **Reintentar integración** (merge fallido): limpia el error para que la cola lo intente de
  nuevo. Resuelve antes el conflicto en el worktree; si no, fallará otra vez.

### Gestionar objetivos

En el detalle de un objetivo, «Gestionar objetivo» permite:

- **Cancelar**: interrumpe los runs, marca las tareas y el objetivo como `cancelled`.
  Conserva todo el historial.
- **Eliminar**: borra el objetivo, sus tareas, runs y leases. Se niega si hay tareas en
  ejecución; cancela primero y espera. El event log conserva `goal_deleted` como auditoría.
- **Reintentar como objetivo nuevo**: crea otro objetivo con el mismo texto para fallidos,
  bloqueados o cancelados.

### Chat, skills y MCP

La pestaña **Chat** muestra, por tarea, el objetivo, la tarea, la respuesta del agente, su
pensamiento (colapsable), las herramientas y los gates. Es de lectura; para intervenir usa
el terminal del worktree.

**Skills/MCP** lista lo que OpenCode tiene registrado. Marca las skills que quieras activar
en las próximas sesiones de tarea; se aplican al crear cada sesión. Los MCP se pueden
conectar, desconectar, añadir en caliente y quitar. Un MCP añadido así vive hasta que
OpenCode se reinicie; para persistirlo, añádelo a la configuración de OpenCode. Un MCP local
ejecuta un comando: instálalo solo si confías en él.
Reiniciar interrumpe las ejecuciones en curso (quedan `interrupted` y sus tareas
`blocked`); el trabajo ya confirmado no se pierde.

### Terminales y código

**Código** abre el repositorio o el worktree de la tarea seleccionada. El editor guarda con
`Ctrl+S`, protege los cambios sin guardar al cambiar de archivo y muestra el diff del
worktree contra la rama base. En la raíz del proyecto puedes hacer **commit manual** con un
mensaje; en un worktree de tarea el commit lo hace el orquestador después de los gates. La
primera pasada no tiene resaltado de sintaxis ni push; para eso está el terminal.

Las terminales viven en el proceso de la app: cerrar la ventana las termina. Mover el PTY
al daemon está pendiente y se documenta como tal; no cierres la ventana en mitad de un
comando largo dentro de una terminal.

Si la app no abre: comprueba que hay sesión gráfica (`WAYLAND_DISPLAY` o `DISPLAY`) y que
`jc-studio` está en el `PATH`. Si abre pero no conecta, revisa `jcs doctor`.

## Configuración

La app es la vía normal. Las variables de entorno siguen existiendo como valor inicial y
para automatización; lo guardado desde la interfaz tiene prioridad.

| Variable | Por defecto | Para qué |
|---|---|---|
| `JCS_SOCKET` | `$XDG_RUNTIME_DIR/jcs/jcsd.sock` | Socket del daemon |
| `JCS_DB` | `$XDG_DATA_HOME/jcs/jcs.db` | Base de datos |
| `JCS_MAX_CONCURRENT` | `4` | Tareas simultáneas (la app lo reparte entre objetivos) |
| `JCS_MODEL` | — | Modelo por defecto (`provider/model`) |
| `JCS_PLANNING_MODEL` | `JCS_MODEL` | Modelo del planificador |
| `JCS_REVIEW` | `0` | Revisor con rúbrica (duplica coste) |
| `JCS_RISK_MAX_LINES` | `400` | Techo de auto-merge por tamaño |
| `JCS_MAX_REPAIR_CYCLES` | `2` | Reparaciones antes de bloquear |
| `JCS_RUN_TIMEOUT_SECS` | `900` | Presupuesto de un run |
| `JCS_CONTEXT_TOKENS` | `1500` | Contexto inyectado (0 lo desactiva) |
| `JCS_MEMORY_SWEEP_SECS` | `300` | Barrido del índice (0 lo desactiva) |
| `JCS_DEFAULT_PACKS` | vacío | Packs si el proyecto no tiene `.jcs/policy.toml` |
| `JCS_PACKS_DIR` | — | Packs extra que sustituyen a los embebidos por id |
| `JCS_EMBEDDING_URL` / `_MODEL` / `_FORMAT` | — | Búsqueda semántica (`ollama`/`openai`) |
| `JCS_TELEGRAM_TOKEN` / `_CHAT_IDS` | — | Bot remoto y allowlist |
| `JCS_ALLOW_SHELL` | `0` | Permite al agente ejecutar shell (no recomendado) |

Los presupuestos y el auto-merge son por proyecto:

```bash
jcs project settings <proyecto> --auto-merge true --budget-usd 10 --budget-tokens 5000000
jcs project settings <proyecto> --clear-budgets
```

## Operación diaria

```bash
jcs ask -p <proyecto> "objetivo"          # planifica, ejecuta, integra
jcs goal <id>                             # tablero del objetivo
jcs logs --follow                         # stream de eventos
jcs tui                                   # interfaz completa
jcs policy render <proyecto> --check      # drift (para CI)
jcs index -p <proyecto>                   # refrescar memoria
```

## Recuperación

**El daemon murió a mitad de un run.** Al arrancar, los runs en vuelo pasan a `interrupted`
y sus tareas a `blocked`; no hay commits parciales. Inspecciona con `jcs status`, corrige lo
que haga falta y `jcs resume <goal>` (o relanza el objetivo).

**Objetivo bloqueado.** `jcs goal <id>` muestra la causa; el event log tiene el detalle
(`needs_input`, `budget_exceeded`, `merge_failed`, `promotion_failed`, `planning_failed`).
Tras resolverla: `jcs resume <id>`.

**Conflicto de merge.** El objetivo queda bloqueado; la rama de la tarea sigue viva. Resuelve
en el worktree (`git worktree list`), o descarta la tarea, y reanuda.

**Worktrees huérfanos.** `git worktree prune` en el repositorio; los directorios viven bajo
`$XDG_DATA_HOME/jcs/worktrees`.

**Backup.** `sqlite3 ~/.local/share/jcs/jcs.db "VACUUM INTO '/ruta/backup.db'"` con el daemon
parado, o usa el endpoint de backup desde código. Copia también `~/.local/share/jcs/secrets.json`
(0600) si quieres conservar el token de Telegram. Restaurar = parar, reemplazar los archivos,
arrancar.

**Socket bloqueado.** Si `jcs doctor` dice que no conecta y el proceso no existe, el daemon lo
detecta como obsoleto al arrancar; si existe otro daemon vivo, el segundo se niega a robar el
socket (por diseño).

## Problemas frecuentes

| Síntoma | Causa probable | Acción |
|---|---|---|
| `agent_unavailable` | OpenCode parado | `opencode service start`, luego `jcs doctor` |
| `agent_auth_failed` | Contraseña de instancia cambiada | Reinicia el servicio; el daemon relee `service.json` |
| Runs fallan en <1 s con 0 tokens | El proveedor del modelo está caído | Prueba otro modelo con `JCS_MODEL` |
| `cannot reach the daemon` | Socket ausente o permisos | `jcsd`, revisa `JCS_SOCKET` y modo 0600 |
| `contract: FAIL` en CI | OpenCode actualizado | Regenera `docs/contracts/` y revisa `CONTRACT.md` |
| Gate `architecture` falla | Import prohibido por `[[boundaries]]` | Corrige el import o ajusta la regla |
| Gate `secrets` falla | Credencial en el worktree | Elimínala, rótala y añade el prefijo a `[secrets] allow` si es un fixture |

## Nada de esto toca tu rama sin permiso

La promoción a la base solo ocurre con `auto_merge`, árbol limpio, rama base chequeada y
fast-forward. Sin eso, el trabajo queda en `jcs/goal/<id>` para que lo revises tú.
