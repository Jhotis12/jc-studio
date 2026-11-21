# Estado del proyecto

Documento vivo. La fuente de verdad del avance son los tests y los scripts, no esta tabla;
si discrepan, gana el código.

Última actualización: 2026-11-21.

## Avance

| Fase | Estimación (días) | Estado | Peso |
|---|---|---|---|
| 0 — Decisiones, entorno y esqueleto | 1,5 | **completa** | 100 % |
| 1 — Walking skeleton vertical | 5 | **completa** | 100 % |
| 2 — Política como código y gates | 7 | **completa** | 100 % |
| 3 — Orquestador durable y TUI | 12 | **casi completa** | 93 % |
| 4 — Memoria: grafo + RAG | 10 | no iniciada | 0 % |
| 5 — Revisión de código y seguridad | 6 | parcial (gates) | 10 % |
| 6 — Telegram | 4 | no iniciada | 0 % |
| 7 — Hardening y distribución | 10 | parcial | 25 % |

- **v1 completo (fases 0–7): ~45 %.**
- **MVP (fases 0–3): ~97 %.**

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

## Evidencia reproducible

| Comando | Qué demuestra |
|---|---|
| `cargo nextest run --workspace` | 220+ tests unitarios, de propiedad y de integración |
| `just e2e` | objetivo → worktree → agente → gates → commit |
| `just e2e-policy` | gate que exige un token: prueba el bucle de reparación |
| `just e2e-plan` | objetivo → plan → DAG → tareas → entregables |
| `just e2e-merge` | DAG → integración → promoción a la base, sin ramas residuales |
| `just chaos` | `kill -9` a mitad de run → `interrupted` + `blocked`, sin commits parciales |
| `just audit` | licencias y avisos de dependencias |

## Riesgos aceptados

Ver [`PENDING.md`](PENDING.md). El único relevante hoy es **RUSTSEC-2026-0459** (SQLite
empaquetado en `sqlx` 0.9), aceptado con precondiciones y dos condiciones de salida.

## Siguiente

Fase 4: índice de código con tree-sitter, grafo en SQLite, búsqueda híbrida (FTS + vectores)
y *context broker* con presupuesto de tokens y citas obligatorias.
