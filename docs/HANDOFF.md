# HANDOFF — leer esto primero

Documento de relevo para un agente (o una persona) que retoma el proyecto **sin contexto
previo**. Si acabas de salir de una compactación: lee esto, luego `docs/STATUS.md`, y no
hagas `git log` completo para reconstruir la historia.

Última actualización: 2026-11-21, tras cerrar la Fase 4.

## Qué es esto

JC Studio: IDE de consola + orquestador multi-agente. Daemon durable (`jcsd`) + cliente
fino (`jcs`) por Unix socket. **OpenCode es el motor de agentes; nosotros somos el plano de
control**: objetivos, DAG, política ejecutable, gates, memoria, git e integración.

## Estado en una frase

**Fases 0–4 completas y verificadas.** El sistema planifica un objetivo, ejecuta tareas en
paralelo con leases, las verifica con gates, las integra en una rama de objetivo y la
promociona a la base; indexa el código y le pasa al agente contexto citado.

- 46 commits, 10 crates, ~21 000 líneas de Rust, **256 tests**.
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest`, `cargo deny check`:
  todo en verde.
- Árbol de trabajo limpio. Nada pendiente de commit.

## Verificación en 60 segundos

```bash
cargo build --bins
cargo nextest run --workspace        # 256 tests
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
| `crates/jcs-cli` | `jcs`: comandos + TUI ratatui |
| `docs/adr` | **Las decisiones y sus motivos** (0001–0009) |
| `docs/STATUS.md` | Avance por fase y qué falta exactamente |
| `docs/PENDING.md` | Riesgos aceptados y decisiones diferidas |
| `scripts/` | Los seis E2E; son la evidencia de las afirmaciones |

Antes de tocar un subsistema, lee su ADR. Las ADR explican por qué algo es como es, y varias
decisiones existen precisamente porque un enfoque obvio falló.

## Siguiente trabajo

**Fase 5 completada** (gates, fitness functions, secretos, revisor, riesgo). **Fase 6
operativa** (Telegram). **Fase 7 parcial**: métricas, systemd, doctor, `contrib/install.sh`, `scripts/package.sh` (tarball Linux
verificado), `docs/RUNBOOK.md` y `scripts/soak.sh` (**verde**). Lo que queda: artefactos
macOS/aarch64, soak largo y Telegram real (ambos los hará el operador).

**Modelos para E2E:** usar `opencode-go/hy4-flash-free` (coste $0, credencial ya activa en
OpenCode). Los modelos del proveedor `opencode` (gratuitos sin credencial) fallaron con
0 tokens en esta máquina; no son una base fiable para tests.

El detalle histórico de las fases 5.x se conserva debajo.

Orden recomendado. Cada paso es un commit que compila y con tests.

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

## Entorno

- OpenCode 2.3.2 en modo *attach* (servicio del usuario). Contrato versionado en
  `docs/contracts/opencode-2.3.2.json`; `scripts/check_contract.py` detecta drift.
- Sin Ollama: búsqueda semántica desactivada; FTS + grafo funcionan.
- Riesgo aceptado: RUSTSEC-2026-0459 (SQLite empaquetado en sqlx 0.9). Condiciones de salida
  en `deny.toml` y `docs/PENDING.md`.
- El grafo del repo está en `graphify-out/`; `graphify query "<pregunta>"` antes de grepear.

## Cómo cerrar una sesión de trabajo

1. `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
2. `cargo nextest run --workspace`
3. `git status` limpio; commits convencionales.
4. Actualizar `docs/STATUS.md` (porcentaje y qué falta) y este HANDOFF si cambia el plan.
5. `graphify update .` y commit del grafo.
