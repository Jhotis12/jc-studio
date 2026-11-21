# Estado del proyecto

Documento vivo. La fuente de verdad del avance son los tests y los scripts, no esta tabla;
si discrepan, gana el código.

Última actualización: 2026-11-21.

Si llegas aquí después de una compactación de contexto, el punto de entrada es
[`HANDOFF.md`](HANDOFF.md): estado, invariantes, mapa del código y siguiente trabajo.

## Avance

| Fase | Estimación (días) | Estado | Peso |
|---|---|---|---|
| 0 — Decisiones, entorno y esqueleto | 1,5 | **completa** | 100 % |
| 1 — Walking skeleton vertical | 5 | **completa** | 100 % |
| 2 — Política como código y gates | 7 | **completa** | 100 % |
| 3 — Orquestador durable y TUI | 12 | **casi completa** | 93 % |
| 4 — Memoria: grafo + RAG | 10 | **casi completa** | 90 % |
| 5 — Revisión de código y seguridad | 6 | **completa** | 100 % |
| 6 — Telegram | 4 | operativa | 85 % |
| 7 — Hardening y distribución | 10 | casi completa (Linux empaquetado) | 80 % |
| 8 — App de escritorio (egui) | 19 | funcional; faltan chat interactivo y PTY en daemon | 94 % |

- **Producto completo (backend + app): ~94 %.**
- **MVP (fases 0–4): ~99 %.**

El porcentaje mide alcance, no riesgo. La tesis arquitectónica (daemon durable, agente real,
worktree aislado, gates, DAG, integración) está validada con tests y scripts.

## Fase 3: qué falta exactamente

- [x] 3.1 Planificador con JSON estricto, validación de dominio y una reparación.
- [x] 3.2 Scheduler durable: dependencias, claims de rutas, prioridad, concurrencia, leases.
- [x] 3.3 Durabilidad: leases con heartbeat, reclamación, recuperación tras `kill -9`.
- [ ] 3.4 Continuidad: presupuestos diarios y bloqueo implementados; **falta** cola de
      objetivos (backlog), modo continuo y horas de silencio.
- [x] 3.5 Merge queue: rebase, re-verificación, fast-forward, conflictos → bloqueo.
- [x] 3.6 TUI: proyectos, objetivos, tareas, detalles, eventos, modal de objetivo, acciones.

## Fase 4: qué falta exactamente

- [x] 4.1 Índice tree-sitter: Rust, TS/JS, Python y SQL (tablas, índices, referencias).
- [x] 4.2 Indexado incremental por hash, al registrar, tras promocionar y por barrido.
- [ ] 4.2b *Watcher* de sistema de archivos: el barrido cada 5 minutos cubre la corrección;
      `notify` queda como optimización.
- [x] 4.3 Búsqueda híbrida: BM25 + grafo + embeddings opcionales (coseno acotado, sin
      extensión vectorial).
- [x] 4.4 Context broker con presupuesto de tokens y citas obligatorias.
- [x] 4.5 Evaluación: recall@3 = 1.0 sobre el fixture.
- [ ] 4.6 Puente a `graphify`: no necesario todavía; el indexador cubre los lenguajes de v1.

## Fase 5: qué falta exactamente

- [x] 5.3 Fitness functions arquitectónicas: reglas `[[boundaries]]` en `.jcs/policy.toml`,
      evaluadas en el worktree y registradas como el gate `architecture`.
- [x] 5.2 Escaneo de secretos: prefijos conocidos + entropía estricta, con allowlist por
      proyecto (`[secrets]`), como gate `secrets` y con el hallazgo redactado.
- [x] 5.1 Revisor con rúbrica: JSON estricto, findings con `archivo:línea`, `error` bloquea,
      opt-in con `JCS_REVIEW=1`, y entra en el bucle de reparación.
- [x] 5.4 Enrutado por riesgo: tamaño del cambio (`JCS_RISK_MAX_LINES`) y, con revisión
      activa, veredicto `review` aprobado por tarea antes de promocionar.

## Fase 6: qué falta exactamente

- [x] Long polling, allowlist de chats, comandos y notificaciones de estados terminales.
- [ ] Aprobaciones firmadas con expiración e idempotencia (hoy se reanuda con `/resume`).
- [ ] Rate limiting y resumen diario.
- Nota: probado con tests de parseo y allowlist, **no contra la API real** de Telegram (no
  hay token en este entorno).

## Fase 8: app de escritorio (egui)

Decisión: cliente principal **instalable**, no una TUI lanzada por consola. Backend intacto.
El razonamiento y las alternativas están en la [ADR 0010](adr/0010-desktop-app-egui.md).

- [x] 8.0 Esqueleto eframe con layout y arranque verificado en Wayland.
- [x] 8.1 Cliente del daemon en la app: hilo dedicado, snapshots `/status`, stream NDJSON con
      cursor, reconexión con backoff y acciones por canal (nunca I/O en el frame).
- [x] 8.2 Tablero real: proyectos, objetivos, DAG, tareas, ejecuciones, gates, política,
      memoria, sistema y consola de eventos. Selección y detalle, compositor de objetivos.
- [x] 8.3 Terminal embebida: `portable-pty` + `vt100`, teclado, scrollback, resize, cierre y
      terminal por proyecto o por worktree de tarea. **Pendiente:** mover el PTY al daemon
      para que la sesión sobreviva al cierre de la ventana.
- [x] 8.4 Ajustes por interfaz: formulario global (modelo, agente, revisión, riesgo, timeout,
      contexto, reparación, concurrencia) y ajustes por proyecto (auto-merge y presupuestos).
      Validación en el daemon, persistencia en `settings`, evento `settings_changed` y aviso
      de «requiere reiniciar». El modelo se elige de un catálogo con precios (44 modelos en
      este backend). **Pendiente:** secretos en el llavero, catálogos de agentes/packs y
      edición estructurada de política.
- [x] 8.5 Empaquetado de escritorio Linux: `jc-studio` en el tarball, `.desktop`, icono,
      instalador que registra la app en el menú. Verificado en un prefijo limpio.
      **Pendiente:** `.deb`/AppImage, instancia única y actualizaciones.
- [x] 8.6 Multi-objetivo real: el scheduler despacha tareas independientes en paralelo, reparte
      la capacidad entre objetivos activos (`max_concurrent / objetivos`, mínimo 1), rota el
      orden para que ninguno acapare y comparte los claims de rutas entre objetivos del mismo
      proyecto. Un objetivo con trabajo en vuelo nunca se bloquea por presupuesto: el techo
      solo impide despachar trabajo nuevo. Prueba: `just e2e-multigoal`.
- [x] 8.7 Progreso por tarea y proceso visible: porcentaje por etapa, barras por tarea en la
      tarjeta del objetivo, grafo del plan con dependencias y aristas, traza en vivo del
      agente (razonamiento, texto, herramientas, gates) y motivo de espera («espera turno»,
      «mismas rutas», «integración de …»). Los eventos de razonamiento se capturan como
      `agent_reasoning`.
- [x] 8.8 Espacio de código (primera entrega): explorador de archivos, editor con guardado,
      creación de archivos, protección de cambios sin guardar y diff del worktree contra la
      rama base. **Pendiente:** resaltado de sintaxis, búsqueda en contenido, renombrar/borrar,
      commit/push desde la UI y chat nativo con OpenCode.
- [x] 8.9 Telegram configurable desde Ajustes: token de escritura única, chats permitidos,
      estado «configurado · …abcd» y aviso de reinicio. El token va a `secrets.json` (0600),
      nunca a SQLite, logs ni API.
- [x] 8.10 Gestión de objetivos: cancelar (detiene runs y tareas, conserva el historial),
      eliminar (bloqueado si hay runs en vuelo, conserva el event log) y reintentar como
      objetivo nuevo. Acciones en la zona de gestión del detalle.
- [x] 8.11 Skills y MCP: catálogo real de OpenCode (346 skills en este entorno), búsqueda,
      selección que se activa al crear la sesión de cada tarea, y lista de servidores MCP con
      conectar/desconectar, añadir en caliente y quitar. Los MCP añadidos en caliente no
      persisten al reiniciar OpenCode; para eso está su configuración.
- [x] 8.12 Chat del agente por tarea: objetivo y tarea como mensajes, respuesta coalescida,
      pensamiento colapsable, herramientas y eventos; solo lectura. El commit manual desde
      el editor está limitado a la raíz del proyecto; los worktrees los commitea el agente.
- [x] 8.13 Progreso honesto: etapas reales por tarea (en cola, agente, verificación,
      integración, completada, merge bloqueado, falló, bloqueada), sin porcentajes falsos;
      barras segmentadas por tarea y resumen apilado del objetivo. Los eventos idénticos
      consecutivos se pliegan con un contador para no inundar la consola.
- [x] 8.14 Detección de objetivos atascados: si no hay tareas en vuelo, ninguna lista y no
      hay merges pendientes, el objetivo pasa a `blocked`/`failed` con `goal_stalled` en vez
      de quedarse `running` para siempre. `ensure_integration_branch` ya no escribe un evento
      por tick.
- [x] 8.18 El Orquestador puede **proponer** un objetivo y el operador convertirlo con un
      botón de confirmación y edición; no crea ni ejecuta nada por su cuenta. El objetivo
      creado pasa por el planificador normal.
- [x] 8.17 Chat del Orquestador: sesión persistente por proyecto, historial en SQLite,
      pestaña propia y guard que rechaza saludos antes de crear un goal. El planificador
      devuelve un plan vacío para conversaciones y un plan vacío bloquea el goal sin lanzar
      agentes. Prueba real: «Hola, ¿de qué trata este repositorio?» respondió y no creó tareas.
- [x] 8.16 Coste real: OpenCode puede reportar `cost: 0` aunque haya tokens. El daemon
      guarda el precio del catálogo por run (migración 0008) y calcula el coste desde los
      tokens cuando el backend reporta 0; un backfill al arrancar corrige los runs antiguos.
      El total de tu proyecto pasó de $0.0000 a **$0.2375** con 1,1 M tokens.
- [x] 8.15 Iteración de tareas: las instrucciones del plan se guardan (migración 0007) y se
      usan en el prompt; una tarea fallida/bloqueada se reintenta con indicaciones y una
      tarea terminada crea una tarea dependiente de iteración. «Reintentar integración»
      limpia el error de merge para que la cola lo reintente tras resolver el conflicto.

Nota: `egui 0.37` rediseñó `eframe::App`; la app está anclada a **0.31** hasta que la nueva
API esté documentada y probada.

## Fase 7: qué falta exactamente

- [x] Métricas Prometheus (`/metrics`, `jcs metrics`), unidad systemd, `jcs doctor`.
- [x] Auditoría de acciones: event log append-only con scopes.
- [x] Soak con techo de presupuesto, auditoría, reactivación y comprobación de fugas
      (`just soak`) — **verde** con `opencode-go/hy4-flash-free` ($0). La versión de 72 h es
      el mismo escenario con un presupuesto mayor.
- [x] Script de instalación (`contrib/install.sh`) y runbook operativo (`docs/RUNBOOK.md`).
- [x] Empaquetado Linux: `scripts/package.sh` (tarball + SHA-256), instalador desde el
      tarball y workflow de release. Verificado en un prefijo limpio.
- [ ] macOS y Linux aarch64 (artefactos adicionales; mismo script, otro target).
- [x] Métricas por proyecto (`jcs_project_cost_usd_total`).
- [ ] Panel de métricas en la TUI clásica (la app de escritorio ya muestra coste y memoria).

## Evidencia reproducible

| Comando | Qué demuestra |
|---|---|
| `cargo nextest run --workspace` | **333 tests** unitarios, de propiedad y de integración |
| `just e2e` | objetivo → worktree → agente → gates → commit |
| `just e2e-policy` | gate que exige un token: prueba el bucle de reparación |
| `just e2e-plan` | objetivo → plan → DAG → tareas → entregables |
| `just e2e-merge` | DAG → integración → promoción a la base, sin ramas residuales |
| `just e2e-memory` | índice → búsqueda citada → incremental → contexto en el prompt |
| `just chaos` | `kill -9` a mitad de run → `interrupted` + `blocked`, sin commits parciales |
| `just soak` | techo de presupuesto → bloqueo auditado → reanudación → sin fugas |
| `just e2e-multigoal` | dos objetivos progresan a la vez, sin locks y ambos terminan |
| `cargo test -p jcs-app` | formularios, estado de la app y un PTY real que ejecuta un comando |
| `./scripts/package.sh` | tarball con daemon, CLI y app; `.desktop` validado e instalación limpia |
| `just audit` | licencias y avisos de dependencias |

## Riesgos aceptados

Ver [`PENDING.md`](PENDING.md). El único relevante hoy es **RUSTSEC-2026-0459** (SQLite
empaquetado en `sqlx` 0.9), aceptado con precondiciones y dos condiciones de salida.

## Siguiente

1. Chat **interactivo** con OpenCode dentro de la app (enviar mensajes a la sesión,
   streaming y diffs), además del chat de lectura y el terminal actual.
2. Mover el PTY al daemon (`/pty`, stream y resize) para que las terminales sobrevivan al
   cierre de la ventana y a un reinicio de la app.
3. Editor: resaltado de sintaxis, búsqueda en contenido, renombrar/borrar y commit/push.
4. Secretos en el llavero del sistema (hoy archivo 0600) y catálogos de agentes/packs.
5. `.deb`/AppImage e instancia única; artefactos macOS y Linux aarch64.
6. Prueba real de Telegram (token del operador) y soak largo de 72 h.
