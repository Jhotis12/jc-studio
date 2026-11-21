# JC Studio

IDE de consola orquestador multi-agente. Un daemon durable (`jcsd`) + un cliente de terminal
(`jcs`) que convierte objetivos en tareas, las ejecuta con agentes reales en worktrees
aislados, las hace pasar gates ejecutables y termina en commits revisados.

**Tesis:** OpenCode es el motor de agentes, no el producto. JC Studio es el plano de control:
objetivos, DAG de tareas, durabilidad, política ejecutable, memoria, git y canales remotos.

```
jcs (TUI/CLI) ─┐
jcsctl        ─┼── UDS (NDJSON + JSON) ──> jcsd ── HTTP/SSE ──> opencode serve
Telegram      ─┘                            │
                                            ├── SQLite (WAL, event log)
                                            ├── git worktrees (1 por tarea)
                                            └── gates (build/lint/test/coverage/arch)
```

## Estado

**Fases 0, 1, 2, 3 y 4: completas.**

Funciona de punta a punta: objetivo → worktree aislado → sesión de agente real → gates
**definidos por la política del proyecto** → bucle de reparación acotado → commit
convencional, con recuperación tras crash y un historial de eventos reanudable.

La política es código versionado: packs en TOML, instrucciones en Markdown, gates
ejecutables, lock con hashes y `render --check` como gate de CI.

El orquestador descompone el objetivo en un DAG, ejecuta tareas en paralelo con leases y
claims de rutas, integra cada commit en una rama de objetivo y promociona a la base cuando
todo está verde. La TUI (`jcs tui`) muestra el tablero y el stream en vivo.

La memoria indexa el código con tree-sitter (símbolos, imports, llamadas, tablas SQL),
responde búsquedas híbridas (BM25 + grafo + embeddings opcionales) y **inyecta contexto
citado** en el prompt de cada tarea.

Lo que **no** hay todavía (fases 5–7): revisor adversarial, Telegram, hardening de 72 h y
empaquetado.

Avance por fase y evidencia: [`docs/STATUS.md`](docs/STATUS.md).
Si retomas el proyecto sin contexto previo: [`docs/HANDOFF.md`](docs/HANDOFF.md).
Riesgos aceptados y decisiones diferidas: [`docs/PENDING.md`](docs/PENDING.md).

## Cómo termina un run

Un run termina por el primero de estos hechos:

| Situación | Resultado |
|---|---|
| El backend reporta `outcome: succeeded` | gates → commit → tarea `succeeded` |
| El backend reporta `failed` / `interrupted` | tarea `failed`, run `failed` |
| El agente hace una **pregunta** (form) | form cancelado, tarea y objetivo `blocked`, con la pregunta en el log |
| Un gate requerido falla | prompt de reparación con la salida del gate, hasta `JCS_MAX_REPAIR_CYCLES`; si persiste, `blocked` |
| Se agota `JCS_RUN_TIMEOUT_SECS` (900 s por defecto) | run `failed`, motivo explícito |
| `jcs abort <run>` | tarea y objetivo `cancelled` |

Una pregunta **no** deja la sesión colgada: se cancela el form y la tarea se aparca como
`blocked` para que un humano decida. Un `blocked` visible es mejor que un `running` eterno.

## Requisitos

- Linux o macOS (Unix domain socket).
- Rust 1.89+ (pinneado en `rust-toolchain.toml`).
- `git` 2.30+.
- OpenCode 2.3.2+ con un servicio en ejecución (`opencode service status`).
- SQLite 3.35+ (embebido vía `sqlx`; el binario `sqlite3` solo hace falta para inspección).

## Instalación

```bash
rustup show                      # instala la toolchain pinneada
cargo build --release

# opción A: usar los binarios del árbol
./target/release/jcsd --help
./target/release/jcs --help

# opción B: instalarlos en ~/.cargo/bin (lo que asume la unidad systemd)
cargo install --path crates/jcs-daemon
cargo install --path crates/jcs-cli
```

## Uso

```bash
# arranca el daemon (o instala contrib/jcsd.service como servicio de usuario)
jcsd &

# registra un proyecto (debe ser un repo git con al menos un commit)
jcs project add ~/code/mi-servicio --name servicio

# lanza un objetivo: crea worktree, sesión de agente, gates y commit
jcs ask servicio "añade validación de email en el endpoint de registro"

# el primer término se toma como proyecto si coincide con uno registrado;
# usa -p para desambiguar
jcs ask -p servicio "otra cosa"

# política del proyecto
jcs policy packs                     # packs disponibles
jcs policy show <proyecto>           # reglas y gates efectivos, con su origen
jcs policy render <proyecto>         # escribe AGENTS.md y .jcs/gates.toml
jcs policy render <proyecto> --check # falla si hay drift (para CI)

# gates, a mano y en el checkout principal (diagnóstico)
jcs gates list <proyecto>
jcs gates run <proyecto> --gate clippy

# TUI
jcs tui

# memoria de código
jcs index                     # indexa el proyecto actual (incremental por hash)
jcs index --force             # re-parsea todo
jcs search -p servicio "validate_email"
jcs memory -p servicio        # estadísticas del índice

# objetivos: planificación y seguimiento
jcs ask -p servicio "objetivo"        # planifica un DAG y lo sigue
jcs ask -p servicio "objetivo" --no-plan   # una sola tarea
jcs goal <goal-id>                    # tablero del objetivo
jcs resume <goal-id>                  # reactiva un objetivo bloqueado

# estado, historial y detalle
jcs status
jcs runs <run-id>
jcs logs --limit 30
jcs logs --follow

# abortar
jcs abort <run-id>
```

### Orquestación

Un objetivo se descompone en tareas con dependencias y claims de rutas. Cada tarea corre en
su propio worktree, cortado de la **rama de integración del objetivo**, y al terminar se
integra ahí mismo (rebase + gates + fast-forward). Los dependientes solo arrancan cuando su
predecesor está **integrado**, no solo terminado.

```bash
# auto-merge: promociona la rama del objetivo a la base al terminar
jcs project settings servicio --auto-merge true

# presupuesto diario por proyecto; al agotarse el objetivo se bloquea, no se ignora
jcs project settings servicio --budget-usd 10 --budget-tokens 5000000
```

`jcs project settings` sin flags muestra la configuración actual. Un objetivo bloqueado se
reactiva con `jcs resume`, y las ramas de tarea se limpian al integrarse.

### Memoria

El índice vive en la misma base de datos del daemon y se actualiza por hash de contenido.
Se dispara al registrar un proyecto, tras cada promoción a la base y en un barrido periódico.
La búsqueda combina BM25, el grafo de llamadas y, si se configura, embeddings.

```bash
export JCS_EMBEDDING_URL=http://127.0.0.1:11434   # Ollama
export JCS_EMBEDDING_MODEL=nomic-embed-text
export JCS_EMBEDDING_FORMAT=ollama                # o openai
```

Sin proveedor, `jcs memory` lo reporta como `semantic search off` y todo lo demás sigue
funcionando. El contexto inyectado se limita con `JCS_CONTEXT_TOKENS` (1500 por defecto, 0
lo desactiva) y cada fragmento lleva su cita `archivo:línea-línea`.

### Política del proyecto

Crea `.jcs/policy.toml` en el repositorio y commítalo:

```toml
packs = ["rust-service", "testing-tdd"]

[rules]
no_unwrap_outside_tests = "permitido en build.rs"

disabled_gates = ["coverage"]

# instrucciones propias, renderizadas después de los packs
instructions = [".jcs/house-rules.md"]

# fitness functions: se evalúan como un gate más
[[boundaries]]
from = "src/domain"
to = "src/infra"
reason = "el dominio no depende de infraestructura"
```

Un `[[boundaries]]` es un gate **ejecutable**: si un archivo bajo `from` importa un módulo
bajo `to`, la verificación falla con `archivo:línea` y el bucle de reparación recibe la
violación. No depende de un modelo ni de un linter.

`jcs policy render` genera `AGENTS.md` y `.jcs/gates.toml`. Los archivos generados llevan
una marca; un `AGENTS.md` escrito a mano **no se sobrescribe** sin `--force`, y al forzarlo
se guarda `AGENTS.md.pre-jcs.bak`.

El run deja un **commit en una rama aislada** (`jcs/<objetivo>/<sufijo>`) dentro de un
worktree propio. No toca tu rama de trabajo. El merge automático llega en la Fase 3;
hasta entonces, revisa el diff y mergea tú:

```bash
git -C ~/code/mi-servicio worktree list
git -C ~/code/mi-servicio diff main...jcs/<rama>
```

## Seguridad por defecto

- El agente trabaja **solo** dentro del worktree de la tarea (`location.directory`).
- **Ejecutar shell está denegado** salvo `JCS_ALLOW_SHELL=1`. La verificación la hace el
  runner de gates, no el proceso que se está verificando.
- Las operaciones destructivas de git no existen en la API (`force-push`, `reset --hard`).
- El socket es `0600` dentro de un directorio `0700`; no hay puerto abierto.
- Las credenciales de OpenCode se leen en memoria, se redactan en `Debug` y nunca se loguean.

## Arquitectura

| Crate | Responsabilidad |
|---|---|
| `jcs-core` | Dominio puro: ids, estados, transiciones válidas. Sin I/O. |
| `jcs-proto` | Protocolo cliente↔daemon, versionado. |
| `jcs-store` | SQLite (WAL) + event log append-only + repositorios. |
| `jcs-agent` | Cliente OpenCode (HTTP + SSE) y descubrimiento del servicio. |
| `jcs-git` | Worktrees, commits convencionales, gates de limpieza. |
| `jcs-gates` | Ejecución de gates: timeout con muerte del grupo, salida acotada, parsers tipados. |
| `jcs-policy` | Packs, composición determinista, render, lock y detección de drift. |
| `jcs-memory` | Índice tree-sitter, búsqueda híbrida y context broker con citas. |
| `jcs-daemon` | `jcsd`: API sobre UDS, ejecutor de tareas, lifecycle. |
| `jcs-cli` | `jcs`: TUI/CLI, cliente fino. |

Decisiones y su justificación: [`docs/adr/`](docs/adr/README.md).
Contrato con OpenCode: [`docs/contracts/`](docs/contracts/).

## Desarrollo

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
just e2e        # end-to-end real (modelo gratuito, requiere opencode servicio activo)
just e2e-policy # gate que exige un token: prueba que el bucle de reparación funciona
just e2e-plan   # descomposición en DAG y ejecución de tareas
just e2e-merge  # DAG + integración + promoción a la rama base
just e2e-memory # índice, búsqueda con cita y contexto inyectado
just chaos      # kill -9 a mitad de run y recuperación
just audit      # licencias y avisos de dependencias
```

`just` agrupa los comandos habituales (`just --list`).

## Principios

1. **Durabilidad antes que elegancia.** Si el proceso muere, el estado sobrevive y el trabajo
   se reanuda o se reporta, nunca se corrompe.
2. **Verificar antes que confiar.** Una regla sin gate es una sugerencia.
3. **Aislamiento por defecto.** Un agente nunca toca la rama compartida directamente.
4. **Presupuesto explícito.** Todo lo autónomo tiene techo de tiempo y de dinero.
5. **Nunca force-push.** Ante duda, escalar a humano.

## Licencia

MIT OR Apache-2.0.
