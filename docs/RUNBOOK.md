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

Ambos instalan en `~/.local/bin` y la unidad en `~/.config/systemd/user/`.

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

## Configuración

| Variable | Por defecto | Para qué |
|---|---|---|
| `JCS_SOCKET` | `$XDG_RUNTIME_DIR/jcs/jcsd.sock` | Socket del daemon |
| `JCS_DB` | `$XDG_DATA_HOME/jcs/jcs.db` | Base de datos |
| `JCS_MAX_CONCURRENT` | `2` | Tareas simultáneas |
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
parado, o usa el endpoint de backup desde código. Restaurar = parar, reemplazar el archivo,
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
