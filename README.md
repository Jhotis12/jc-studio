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

Fase 0 (decisiones + esqueleto) y Fase 1 (walking skeleton vertical) en progreso.
El alcance de v1 está congelado en `docs/adr/`. Nada de UI web, PRs automáticos ni Windows nativo.

## Requisitos

- Linux o macOS (Unix domain socket).
- Rust 1.85+ (pinneado en `rust-toolchain.toml`).
- `git` 2.30+.
- OpenCode 2.3.2+ con un servicio en ejecución (`opencode service status`).
- SQLite 3.35+ (embebido vía `sqlx`; el binario `sqlite3` solo hace falta para inspección).

## Instalación

```bash
rustup show                      # instala la toolchain pinneada
cargo build --release
./target/release/jcsd --help
./target/release/jcs --help
```

## Uso

```bash
# arranca el daemon (o usa systemd --user, ver contrib/)
jcsd &

# registra un proyecto (debe ser un repo git)
jcs project add ~/code/mi-servicio --name servicio

# lanza un objetivo: crea worktree, sesión de agente, gates y commit
jcs ask servicio "añade validación de email en el endpoint de registro"

# estado y stream
jcs status
jcs logs --follow
```

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
