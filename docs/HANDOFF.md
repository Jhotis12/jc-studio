# HANDOFF — leer esto primero

Documento de relevo para un agente (o una persona) que retoma el proyecto **sin contexto
previo**. Si acabas de salir de una compactación: lee esto, luego `docs/STATUS.md`, y no
hagas `git log` completo para reconstruir la historia.

Última actualización: 2026-11-21, tras la primera entrega de la app de escritorio.

## Qué es esto

JC Studio: orquestador multi-agente con daemon durable (`jcsd`) y dos clientes finos:
la **app de escritorio instalable** `jc-studio` (egui, con terminales embebidas) y el CLI
`jcs` (scripting y TUI para SSH). **OpenCode es el motor de agentes; nosotros somos el plano
de control**: objetivos, DAG, política ejecutable, gates, memoria, git e integración.

## Estado en una frase

**Fases 0–7 completas; Fase 8 funcional.** El sistema planifica un objetivo, ejecuta tareas
en paralelo con leases, las verifica con gates, las integra en una rama de objetivo y la
promociona a la base; indexa el código, le pasa al agente contexto citado y la app de
escritorio permite operar todo sin tocar la consola.

- 12 crates, **343 tests**, árbol limpio.
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest`, `cargo deny check`:
  todo en verde.
- Soak verde; tarball Linux con `jcsd`, `jcs` y `jc-studio` (`./scripts/package.sh`).
- Lo que queda de la Fase 8: chat nativo con OpenCode, PTY en el daemon, resaltado de
  sintaxis/búsqueda/commit en el editor, llavero del sistema y `.deb`/AppImage.

## Verificación en 60 segundos

```bash
cargo build --bins
cargo nextest run --workspace        # 343 tests
python3 scripts/check_contract.py    # contrato de OpenCode
cargo deny check                     # licencias y avisos
```

End-to-end reales (usan un modelo gratuito y un repo temporal; requieren
`opencode service status` funcionando):

```bash
just e2e          # objetivo -> worktree -> agente -> gates -> commit
just e2e-policy   # gate que exige un token: prueba el bucle de reparación
just e2e-plan     # descomposición en DAG
just e2e-merge    # DAG + integración + promoción a main
just e2e-memory   # índice + búsqueda citada + contexto en el prompt
just chaos        # kill -9 a mitad de run y recuperación
just soak         # presupuesto, auditoría, reanudación y fugas
just e2e-multigoal # dos objetivos a la vez, reparto justo, sin locks
cargo test -p jcs-app                 # estado de la app y PTY real
```

Los seis deben pasar. Si uno falla, ese es el trabajo, no una fase nueva.

## Invariantes que NO se deben romper

1. **Un solo dueño por tarea.** El lease (`leases`) se adquiere antes de despachar; el
   ejecutor aborta si pierde el heartbeat. Sin esto hay dos agentes en el mismo worktree.
2. **Orden de cierre:** run terminal → estado de tarea → estado de objetivo. Invertirlo
   hace que el cliente salga antes de ver el resultado (bug ya sufrido).
3. **Integrar antes de despachar.** Un dependiente se corta de la rama de integración del
   objetivo y solo arranca cuando su predecesor está `succeeded` **y** `merged_at` puesto.
4. **`blocked` no es `failed`.** Una pregunta del agente, un conflicto de merge o un
   presupuesto agotado bloquean; el trabajo sigue siendo válido.
5. **Un gate vacío no es un pase.** `GateSet::all_required_passed` devuelve false sin gates.
6. **Nunca force-push, nunca resolver conflictos solos, nunca tocar la base con árbol sucio.**
7. **`AGENTS.md` escrito a mano nunca se sobrescribe sin `--force`** (y con backup).
8. **Credenciales de OpenCode**: solo en memoria, redactadas en `Debug`, nunca en logs.
9. **`session.outcome` persiste** entre turnos; solo es válido si `time.idle > time.busy`.
10. **FTS de memoria es por proyecto**: la misma ruta relativa existe en todos los repos.

## Mapa del código

| Ruta | Qué hay |
|---|---|
| `crates/jcs-core` | Dominio puro: ids, estados, eventos, `Plan`/`TaskSpec` |
| `crates/jcs-store` | SQLite, migraciones 0001–0005, event log, DAG, leases, integración |
| `crates/jcs-agent` | Cliente OpenCode, SSE a mano, descubrimiento, forms/permisos |
| `crates/jcs-git` | Worktrees, commits convencionales, rebase, ancestría |
| `crates/jcs-gates` | Ejecución de gates con timeout y muerte de grupo, parsers |
| `crates/jcs-policy` | Packs TOML, composición, render determinista, lock/drift |
| `crates/jcs-memory` | tree-sitter, grafo, FTS5, embeddings opcionales, context broker |
| `crates/jcs-daemon` | `jcsd`: HTTP/UDS, scheduler, planner, merge queue, executor |
| `crates/jcs-client` | Cliente del socket: peticiones y stream NDJSON (lo usan CLI y app) |
| `crates/jcs-cli` | `jcs`: comandos + TUI ratatui |
| `crates/jcs-app` | `jc-studio`: app egui, estado de UI, terminal PTY, ajustes |
| `docs/adr` | **Las decisiones y sus motivos** (0001–0010) |
| `docs/STATUS.md` | Avance por fase y qué falta exactamente |
| `docs/PENDING.md` | Riesgos aceptados y decisiones diferidas |
| `scripts/` | Los seis E2E; son la evidencia de las afirmaciones |

Antes de tocar un subsistema, lee su ADR. Las ADR explican por qué algo es como es, y varias
decisiones existen precisamente porque un enfoque obvio falló.

## Siguiente trabajo

**Fases 5–7 completas** (gates, secretos, revisor, riesgo, Telegram operativo, soak verde,
tarball Linux). **Fase 8 funcional**: la app de escritorio ya opera proyectos, objetivos,
detalle, política, memoria, ajustes, eventos y terminales.

Orden recomendado. Cada paso es un commit que compila y con tests:

1. **Chat interactivo con OpenCode.** Enviar mensajes a la sesión desde el chat de la tarea
   (hoy es de lectura), con streaming y diffs. El cliente ya tiene el lector NDJSON y el
   daemon ya habla el contrato; falta el endpoint de mensaje y el control de concurrencia con
   el ejecutor.
2. **PTY en el daemon.** Endpoints `/pty`, input, resize, stream y cierre; la app cambia
   `terminal::Terminals` por un cliente de esos endpoints. Así cerrar la ventana no mata un
   agente a mitad de tarea.
3. **Editor de segunda pasada.** Resaltado de sintaxis, búsqueda en contenido, renombrar y
   borrar archivos, y commit/push desde la UI. La primera pasada (árbol, edición, guardado,
   archivos nuevos y diff) ya está.
4. **Secretos y catálogos.** Llavero del sistema como opción preferente sobre el archivo
   0600; catálogos de agentes y packs, además de los modelos.
5. **Distribución.** `.deb`/AppImage, instancia única y artefactos macOS/aarch64.
6. **Operador:** prueba real de Telegram (token/chat id) y soak de 72 h
   (`SOAK_BUDGET_TOKENS=<alto> ./scripts/soak.sh`).

**Modelos para E2E:** usar `opencode-go/hy4-flash-free` (coste $0, credencial ya activa en
OpenCode). Los modelos del proveedor `opencode` (gratuitos sin credencial) fallaron con
0 tokens en esta máquina; no son una base fiable para tests.

El detalle histórico de las fases 5.x se conserva debajo.

### 5.3 Fitness functions arquitectónicas — HECHO

`BoundaryRule` en `jcs-core`, comprobador en `jcs-memory::boundaries`, reglas en
`.jcs/policy.toml` (`[[boundaries]]`) y veredicto integrado en `verify.rs` como gate
`architecture`. Pendiente menor: permitir declarar boundaries en los packs.

### 5.1 Revisor con rúbrica — HECHO

`crates/jcs-daemon/src/reviewer.rs`: sesión dedicada, JSON estricto, findings con
`archivo:línea`, `error` bloquea, `request_changes` sin `error` se degrada a aprobación.
Opt-in con `JCS_REVIEW=1`; un fallo del revisor **no** bloquea trabajo que ya pasó gates
(se registra como `errored`). Pendiente: revisión adversarial (segunda pasada).

### 5.2 Escaneo de secretos — HECHO

`jcs-gates::secrets` (prefijos conocidos + entropía) con `ScanOptions::allow_prefixes`,
configurado por `[secrets]` en `.jcs/policy.toml` y registrado como gate `secrets` en
`verify.rs`. El hallazgo se redacta: nunca se imprime la credencial.

El gate de tarea usa `jcs-git::Repo::working_changed_lines()` y `scan_targets`: solo las
líneas añadidas contra `HEAD` y los archivos no rastreados. Omite lockfiles, `graphify-out/`,
`dist/`, `target/` y minificados; la heurística ignora nombres con `/ - _ .`. La razón está
en la [ADR 0017](adr/0017-secrets-scope.md). `scan_secrets(root, options)` sigue disponible
para auditar el árbol completo.

### 5.3 Revisor con rúbrica (modelo)

Patrón ya probado en `crates/jcs-daemon/src/planner.rs`: sesión dedicada, salida JSON
estricta, validación de dominio y **una** reparación. Nuevo `crates/jcs-daemon/src/reviewer.rs`:

- Entrada: diff del worktree (`git diff` contra la base), reglas de política, salida de gates.
- Salida validada: `{ verdict: "approve"|"request_changes", findings: [{severity, file, line, message}], summary }`.
- Debe citar `archivo:línea`; sin cita, el finding se rechaza.
- Se ejecuta tras los gates y antes del commit; `request_changes` entra en el mismo bucle de
  reparación que un gate rojo (comparte presupuesto `JCS_MAX_REPAIR_CYCLES`).
- Opt-in al principio (`JCS_REVIEW=1`) para no duplicar coste en cada tarea.

### 5.4 Enrutado por riesgo — HECHO

`merge::promote` rechaza promocionar si el diff supera `JCS_RISK_MAX_LINES` (400 por
defecto) o si, con `JCS_REVIEW=1`, alguna tarea no tiene un gate `review` aprobado. El
objetivo queda `Blocked` con el motivo, nunca a medias.

### 5.4b Enrutado por riesgo (histórico)

En `crates/jcs-daemon/src/merge.rs`: antes de promocionar, calcular riesgo =
gates + findings del revisor + tamaño del diff + rutas tocadas vs `paths` declarados. Si no
es "bajo", el objetivo pasa a `Blocked` con el motivo y espera `jcs resume` (decisión humana
registrada en el event log).

## Fase 6 (Telegram) — estado

`crates/jcs-daemon/src/telegram.rs`: long polling, allowlist estricta, `/status`,
`/projects`, `/goal`, `/resume`, `/ask`, y notificaciones desde el event log. Configuración:
`JCS_TELEGRAM_TOKEN` y `JCS_TELEGRAM_CHAT_IDS`. **No probado contra la API real** (sin
token): antes de confiar en él, levantar el bot con un token de prueba y verificar
`getUpdates`, allowlist y un `/status`.

## Fase 8 — app de escritorio (egui)

El cliente principal es una **app instalable** orientada a agentes, no una TUI lanzada por
consola. La decisión y las alternativas están en la
[ADR 0010](adr/0010-desktop-app-egui.md). El backend no cambia.

- Crate `crates/jcs-app`, binario `jc-studio`, `eframe`/`egui` **0.31** (0.37 cambió
  `eframe::App` a `ui(&mut Ui)` y movió los paneles; no usar 0.37 sin adaptar).
- `src/daemon.rs`: hilo dedicado con runtime Tokio; comandos por canal y actualizaciones por
  `std::sync::mpsc`. Snapshots `/status` cada 2 s + `/events?since` con reconexión.
  Las acciones lentas (gates, indexación) se lanzan en su propia tarea.
- `src/state.rs`: espejo del snapshot; selección, coste por objetivo, deduplicación de
  eventos por `seq`, consola acotada a 1000 líneas.
- `src/ui/`: shell responsive (tres paneles en ventana ancha, pestaña Detalle en estrecha),
  tablero, detalle, política, memoria, sistema, ajustes y ventanas modales.
- `src/terminal.rs`: `portable-pty` + `vt100` (no `egui_term`, anclado a egui viejo).
  Teclado, scrollback con rueda, resize del PTY y color 256. Las sesiones viven en el proceso
  de la app; **mover el PTY al daemon es el siguiente paso** para que sobrevivan al cierre.
- Ajustes: formulario global y por proyecto; el daemon valida, persiste en `settings` y
  emite `settings_changed`. `max_concurrent` se aplica al reiniciar y la UI lo avisa.
  Los secretos **no** van a SQLite: quedan pendientes del llavero.
- Empaquetado: `scripts/package.sh` incluye los tres binarios, `.desktop`, icono y
  `install.sh`; el instalador registra la app en el menú. Validado en un prefijo limpio.
- Multi-objetivo: `scheduler::tick` reparte la capacidad (`max_concurrent / objetivos
  activos`, mínimo 1), rota el orden de objetivos y comparte los claims de rutas entre
  objetivos del mismo proyecto. Un objetivo con tareas en vuelo no se bloquea por
  presupuesto; el techo solo impide despachar trabajo nuevo.
- Progreso por tarea: `TaskProgress` por etapa (en cola 0 %, agente 45 %, verificando 80 %,
  completada 100 %), barras por tarea en la tarjeta del objetivo, grafo del plan y traza del
  agente (razonamiento, texto, herramientas, gates). El motivo de espera se calcula en la UI
  a partir de dependencias, claims y capacidad; el daemon sigue siendo la autoridad.
- Editor: pestaña «Código» con árbol, edición, guardado, archivos nuevos, diff del worktree
  contra la base y protección de cambios sin guardar. La pestaña abre por defecto el worktree
  de la tarea seleccionada; si esa tarea aún no tiene, muestra el más reciente. Un selector
  permite volver a la raíz y «Ver cambios» (en el detalle) carga el diff. Si el worktree ya se
  integró y se eliminó, cae al más reciente con aviso.
  Es edición local, no orquestación: el daemon no participa en leer/escribir archivos.
- Actividad: `State::task_activity` guarda la última acción por tarea fuera de la consola
  acotada (razonando, escribiendo, herramienta, gate, commit, merge) y el tablero/detalle
  muestran su edad. El evento de integración solo se emite cuando la rama está estrictamente
  detrás de la base; la igualdad no cuenta.
- Telegram: token de escritura única en `secrets.json` (0600), chats permitidos en `settings`,
  y aviso de reinicio. La API solo devuelve `telegram_configured` y los últimos cuatro
  caracteres.
- Objetivos: cancelar (detiene runs/tareas), eliminar (409 si hay runs en vuelo; borra
  tareas/runs/leases por cascada y conserva el event log con `goal_deleted`) y reintentar
  como objetivo nuevo.
- Chat: pestaña «Chat» por tarea; objetivo y tarea como mensajes, deltas de texto agrupados,
  pensamiento colapsable, herramientas y eventos. Las herramientas se decodifican de la forma
  real de OpenCode 2.3.2: el nombre llega en `session.tool.input.started`, la llamada en
  `session.tool.called` (sin campo `tool`) y el resultado en `session.tool.success`
  (`content`). El ejecutor cachea el nombre por `call_id`. Es de lectura; enviar mensajes a
  la sesión es el siguiente paso.
- Skills/MCP: `GET /catalog/skills` y `/catalog/mcp` (OpenCode), selección de skills en
  `settings.agent_skills` que el ejecutor activa antes del prompt, y MCP conectar/desconectar/
  añadir en caliente/quitar. La consulta correcta de OpenCode es
  `location[directory]=...`, no `directory=...`.
- Editor: commit manual desde la raíz del proyecto; los worktrees de tarea los commitea el
  orquestador tras los gates.
- Progreso: `TaskStage` (en cola, agente, verificando, integrando, completada, merge
  bloqueado, falló, bloqueada) con barra segmentada; nunca un porcentaje inventado. El
  objetivo usa una tira apilada por tarea, no un fill único que se vuelve azul al arrancar
  una tarea.
- Estancamiento: si no hay in-flight, `ready_tasks` está vacío, no hay merges pendientes y
  quedan pending, el objetivo se marca `blocked`/`failed` (`goal_stalled`). Antes quedaba
  `running` para siempre cuando una tarea dependía de otra bloqueada o con merge fallido.
- Iteración: `tasks.instructions` (migración 0007) guarda la descripción del plan y el
  feedback. `POST /tasks/{id}/iterate` reintenta failed/blocked con instrucciones o crea una
  tarea dependiente para una succeeded. `merge-retry` limpia `merge_error`.
- Streaming: el chat del Orquestador responde `POST` en milisegundos y emite
  `orchestrator_text`/`orchestrator_reasoning` cada ~250 ms; el mensaje final se persiste y
  llega `orchestrator_finished`. Las tareas ya usaban `agent_text` cada 300 ms. Depende del
  proveedor: el gratuito probado bufferiza el texto hasta el final, `deepseek-v4.1-flash`
  streama. Markdown local en `ui/markdown.rs` (sin `egui_commonmark`).
- Orquestador: chat persistente por proyecto (`orchestrator_messages`/`orchestrator_sessions`,
  migración 0009), pestaña propia, prompt de rol sin modificar archivos. Cada respuesta
  ofrece «Crear objetivo con esta respuesta»: formulario editable + confirmación; el
  Orquestador no llama a `/ask` solo. `POST /ask` rechaza saludos exactos
  (`not_an_objective`) y el planificador devuelve `{"tasks":[]}` para conversaciones; un
  plan vacío bloquea el goal sin lanzar agentes. El chat del Orquestador no es la sesión del
  planificador de cada goal.
- Coste: `runs` guarda el modelo y el precio por millón (migración 0008). Si OpenCode reporta
  `cost=0` pero hay tokens, se calcula desde el catálogo; un backfill al arrancar corrige los
  runs antiguos. El total actual cubre runs de tareas; planificador y revisor aún no.

## Trampas ya encontradas (no repetirlas)

- `git show --name-only <branch>` lista el **último commit**, no el árbol: usar `ls-tree`.
- sqlx 0.9 rechaza SQL construido con `format!`: usar literales o `QueryBuilder`.
- `memory_edges` usa `src_path`, no `path` (costó un debugging entero).
- Un worktree **dentro** del repo lo ensucia: viven en `$XDG_DATA_HOME/jcs/worktrees`.
- Clap no acepta un posicional opcional antes de uno requerido: usar flags.
- Un CLI debe poner `SIGPIPE` por defecto o `jcs logs | head` paniquea.
- El stream de OpenCode **no** usa el campo SSE `event:`; el tipo va dentro del JSON.
- El planificador debe leer el mensaje final por REST: suscribirse después del prompt pierde
  respuestas rápidas.
- egui 0.31 usa `Frame::new()` y `Margin`; `Frame` no expone `response`, hay que capturar el
  `InnerResponse` que devuelve `show`.
- `vt100::Parser::new` y `set_size` piden `NonZeroU16`, no `u16`.
- La API de política y memoria recibe **nombre o ruta** de proyecto, no el id (`prj_...`).
  La app debe resolver el nombre desde el snapshot antes de pedir.
- Un panel derecho de 390 px más uno izquierdo de 250 no caben en una ventana de 476 px:
  la app oculta paneles según el ancho y convierte el detalle en pestaña.
- El progreso de una tarea no es un porcentaje: 45 % → 100 % era una mentira de la UI. Usar
  etapas y segmentos; el usuario detectó el salto azul.
- Un objetivo con una tarea pending que depende de una bloqueada/merge-fallida **no**
  termina solo. Hay que detectar el estancamiento; `settled()` no basta.
- `ensure_integration_branch` corre cada tick: emitir un evento siempre inunda el log.
- Las instrucciones del plan se perdían: sin `tasks.instructions`, el agente solo veía el
  título. La migración 0007 las conserva para prompts e iteraciones.
- SQLite no espera a una transacción diferida que empieza leyendo y luego escribe: devuelve
  `SQLITE_BUSY` sin llamar al busy handler. Con varios agentes eso fallaba como «database is
  locked». El pool usa **una sola conexión**; SQLite ya tiene un solo escritor.
- El techo de presupuesto se comprueba por objetivo: si el objetivo tiene tareas en vuelo,
  se deja terminar y se corta solo el despacho nuevo. Bloquearlo a mitad de run fue un bug.
- `/status` debe enriquecer las tareas con dependencias, `path_claims` y `merged_at`; sin eso
  el grafo de la app sale sin aristas y no se puede explicar la espera.
- El token de Telegram no está en `settings`: `secrets.json` con 0600. No imprimirlo en
  logs, errores ni respuestas.
- OpenCode puede devolver `cost: 0` con tokens > 0 (suscripción/créditos). No confiar en ese
  0: usar el precio del catálogo por millón y los tokens, y dejar los modelos gratuitos en 0.
- No interpretar un `*` suelto como énfasis (`*.rs` es un glob); el renderizador exige
  cierre y trata `[`/backtick sin pareja como literales para no entrar en bucles.
- El streaming del texto depende del modelo; no prometer token a token si el proveedor
  bufferiza. El razonamiento en vivo no se guarda en la conversación.
- No volver a escanear todo el worktree en el gate de secretos: los lockfiles y los nombres
  de modelo hicieron que dos tareas intentaran "arreglar" archivos ajenos y una acabara
  evadiendo el escáner. Ver ADR 0017.
- El coste del planificador, del revisor y del chat del Orquestador no está asociado a un run
  de tarea; hoy no entra en el total. Está anotado como pendiente.
- «Hola» en el compositor creó 4 tareas leyendo OpenSpec: el planificador no distingue una
  conversación de un objetivo. Guard exacto + plan vacío + chat separado, y no dar por hecho
  que el modelo respeta el prompt sin refuerzo.

## Entorno

- OpenCode 2.3.2 en modo *attach* (servicio del usuario). Contrato versionado en
  `docs/contracts/opencode-2.3.2.json`; `scripts/check_contract.py` detecta drift.
- Sin Ollama: búsqueda semántica desactivada; FTS + grafo funcionan.
- Riesgo aceptado: RUSTSEC-2026-0459 (SQLite empaquetado en sqlx 0.9). Condiciones de salida
  en `deny.toml` y `docs/PENDING.md`.
- El grafo del repo está en `graphify-out/`; `graphify query "<pregunta>"` antes de grepear.
- La app necesita sesión gráfica (Wayland o X11). Para probarla sin tocar el daemon real:
  `JCS_SOCKET=/tmp/prueba.sock ./target/debug/jc-studio`, con un `jcsd` en el mismo socket.
- Modelo de los E2E y del soak: `opencode-go/hy4-flash-free` ($0 con la credencial activa).

## Cómo cerrar una sesión de trabajo

1. `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
2. `cargo nextest run --workspace`
3. `git status` limpio; commits convencionales.
4. Actualizar `docs/STATUS.md` (porcentaje y qué falta) y este HANDOFF si cambia el plan.
5. `graphify update .` y commit del grafo.
