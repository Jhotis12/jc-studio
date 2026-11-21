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

**Fase 0 (decisiones + esqueleto) y Fase 1 (walking skeleton vertical): completas.**

Funciona de punta a punta: daemon durable → worktree aislado → sesión de agente real →
stream de eventos → gates → commit convencional, con recuperación tras crash y un
historial de eventos reanudable.

Lo que **no** hay todavía (fases 2–7): compilador de políticas y packs, descomposición de
objetivos en DAG, TUI ratatui, grafo de código/RAG, Telegram, merge queue.

Riesgos aceptados y decisiones diferidas: [`docs/PENDING.md`](docs/PENDING.md).

## Cómo termina un run

Un run termina por el primero de estos hechos:

| Situación | Resultado |
|---|---|
| El backend reporta `outcome: succeeded` | gates → commit → tarea `succeeded` |
| El backend reporta `failed` / `interrupted` | tarea `failed`, run `failed` |
| El agente hace una **pregunta** (form) | form cancelado, tarea y objetivo `blocked`, con la pregunta en el log |
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

# estado, historial y detalle
jcs status
jcs runs <run-id>
jcs logs --limit 30
jcs logs --follow

# abortar
jcs abort <run-id>
```

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
