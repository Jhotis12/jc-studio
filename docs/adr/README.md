# ADRs — JC Studio

Registro de decisiones arquitectónicas. Cada ADR captura una decisión **cara de revertir**:
contexto, alternativas evaluadas, decisión, consecuencias y cómo se verifica que sigue vigente.

Formato: [MADR](https://adr.github.io/madr/) simplificado. Numeración inmutable: un ADR
nunca se renumera; si una decisión cambia, se escribe uno nuevo y el anterior pasa a
`Superseded by NNNN`.

| # | Decisión | Estado |
|---|---|---|
| [0001](0001-daemon-thin-client.md) | Daemon `jcsd` + cliente fino por Unix socket | Accepted |
| [0002](0002-opencode-http-api-backend.md) | OpenCode v2 HTTP API como backend de agentes (modo attach) | Accepted |
| [0003](0003-sqlite-event-log.md) | SQLite + sqlx con event log append-only como fuente de verdad | Accepted |
| [0004](0004-policy-as-code-with-gates.md) | Política como código con doble capa: instrucciones + gates ejecutables | Accepted |
| [0005](0005-worktree-per-task.md) | Un worktree por tarea y git vía CLI (no libgit2/gix) | Accepted |
| [0006](0006-client-streaming-ndjson.md) | TUI ratatui y streaming NDJSON sobre UDS (no SSE para el cliente) | Accepted |

## Cómo añadir una ADR

1. Copia la estructura del ADR más reciente.
2. Numera con el siguiente entero de cuatro dígitos.
3. Estado inicial: `Proposed`. Pasa a `Accepted` al mergear la implementación que la aplica.
4. Si contradice una anterior, marca la anterior como `Superseded by NNNN` y explica el delta.

## Regla de oro

Si una decisión no tiene una sección **Verificación**, es una opinión, no una decisión.
La verificación debe ser un comando, un test o un gate que falle cuando la decisión se rompe.
