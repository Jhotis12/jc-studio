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
| 8 — App de escritorio (egui) | 15 | funcional; faltan PTY en daemon y catálogos | 75 % |

- **Producto completo (backend + app): ~85 %.**
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
| `cargo nextest run --workspace` | **305 tests** unitarios, de propiedad y de integración |
| `just e2e` | objetivo → worktree → agente → gates → commit |
| `just e2e-policy` | gate que exige un token: prueba el bucle de reparación |
| `just e2e-plan` | objetivo → plan → DAG → tareas → entregables |
| `just e2e-merge` | DAG → integración → promoción a la base, sin ramas residuales |
| `just e2e-memory` | índice → búsqueda citada → incremental → contexto en el prompt |
| `just chaos` | `kill -9` a mitad de run → `interrupted` + `blocked`, sin commits parciales |
| `just soak` | techo de presupuesto → bloqueo auditado → reanudación → sin fugas |
| `cargo test -p jcs-app` | formularios, estado de la app y un PTY real que ejecuta un comando |
| `./scripts/package.sh` | tarball con daemon, CLI y app; `.desktop` validado e instalación limpia |
| `just audit` | licencias y avisos de dependencias |

## Riesgos aceptados

Ver [`PENDING.md`](PENDING.md). El único relevante hoy es **RUSTSEC-2026-0459** (SQLite
empaquetado en `sqlx` 0.9), aceptado con precondiciones y dos condiciones de salida.

## Siguiente

1. Mover el PTY al daemon (`/pty`, stream y resize) para que las terminales sobrevivan al
   cierre de la ventana y a un reinicio de la app.
2. Secretos en el llavero del sistema y catálogos de modelos/agentes/packs en los ajustes.
3. `.deb`/AppImage e instancia única; artefactos macOS y Linux aarch64.
4. Prueba real de Telegram (token del operador) y soak largo de 72 h.
