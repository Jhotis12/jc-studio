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
| 7 — Hardening y distribución | 10 | parcial (falta empaquetado) | 65 % |

- **v1 completo (fases 0–7): ~90 %.**
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

## Fase 7: qué falta exactamente

- [x] Métricas Prometheus (`/metrics`, `jcs metrics`), unidad systemd, `jcs doctor`.
- [x] Auditoría de acciones: event log append-only con scopes.
- [x] Soak con techo de presupuesto, auditoría, reactivación y comprobación de fugas
      (`just soak`) — **verde** con `opencode-go/hy4-flash-free` ($0). La versión de 72 h es
      el mismo escenario con un presupuesto mayor.
- [x] Script de instalación (`contrib/install.sh`) y runbook operativo (`docs/RUNBOOK.md`).
- [ ] Empaquetado `cargo dist` para binarios multiplataforma.
- [ ] Endpoint de métricas por proyecto (hoy agregado) y panel en la TUI.

## Evidencia reproducible

| Comando | Qué demuestra |
|---|---|
| `cargo nextest run --workspace` | 220+ tests unitarios, de propiedad y de integración |
| `just e2e` | objetivo → worktree → agente → gates → commit |
| `just e2e-policy` | gate que exige un token: prueba el bucle de reparación |
| `just e2e-plan` | objetivo → plan → DAG → tareas → entregables |
| `just e2e-merge` | DAG → integración → promoción a la base, sin ramas residuales |
| `just e2e-memory` | índice → búsqueda citada → incremental → contexto en el prompt |
| `just chaos` | `kill -9` a mitad de run → `interrupted` + `blocked`, sin commits parciales |
| `just audit` | licencias y avisos de dependencias |

## Riesgos aceptados

Ver [`PENDING.md`](PENDING.md). El único relevante hoy es **RUSTSEC-2026-0459** (SQLite
empaquetado en `sqlx` 0.9), aceptado con precondiciones y dos condiciones de salida.

## Siguiente

Fase 5: revisión de código con rúbrica y salida estructurada, suite estática completa,
fitness functions arquitectónicas y enrutado por riesgo.
