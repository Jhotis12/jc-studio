# 0002 — OpenCode v2 HTTP API como backend de agentes (modo attach)

- **Estado:** Accepted
- **Fecha:** 2026-11-21
- **Decisores:** operador (jhotis)
- **Reemplaza a:** versión inicial de esta ADR que proponía un `opencode serve` por proyecto.

## Contexto

JC Studio no debe implementar un runtime de agentes: eso es un producto en sí mismo
(loop de herramientas, streaming, compactación de contexto, permisos, MCP, skills). Lo que
falta en el mercado —y lo que aporta este proyecto— es el **plano de control**: objetivos,
DAG de tareas, durabilidad, políticas, gates, memoria, git y canales remotos.

OpenCode v2.3.2 en esta máquina expone 142 paths / 173 operaciones HTTP documentadas
(copia versionada en `docs/contracts/opencode-2.3.2.json`), incluyendo sesiones con
`location.directory`, prompts, permisos interactivos, forms, diffs por turno, worktrees,
VCS, PTY y un stream SSE con eventos granulares.

## Alternativas consideradas

1. **ACP (`opencode acp`).** Protocolo pensado para editores; superficie menor y menos
   observable que la HTTP API. Sin acceso directo a coste por sesión, permisos o diffs.
2. **Wrapper de CLI (`opencode run --format json`).** Frágil: sin stream estructurado de
   herramientas, sin interacción de permisos, sin estado de sesión consultable.
3. **Un `opencode serve` por proyecto.** El plan inicial. Aislamiento por proceso, pero
   N servidores compiten por la misma base de estado local y multiplican memoria y arranque.
4. **Un único servidor compartido, sesiones con `location.directory`.** OpenCode v2 es
   multi-proyecto por diseño (`/api/project`, `session.create { location }`).

## Decisión

Usar la **API HTTP v2** de OpenCode como backend inicial, con un trait `AgentBackend` en
`jcs-agent` para añadir otros backends (ACP, Claude Code, Codex) sin tocar el orquestador.

**Modo por defecto: `attach`.** Nos conectamos al servicio OpenCode ya en ejecución
(`opencode service status` → URL; credencial en `~/.config/opencode/service.json`), con
overrides por entorno `JCS_OPENCODE_URL` / `JCS_OPENCODE_PASSWORD`. Si el servicio no está
corriendo, el daemon lo arranca con `opencode service start` y espera a `health`.

El aislamiento entre proyectos se hace con **`location.directory` por sesión**, no con un
proceso por proyecto. El modo `spawn` (proceso propio en puerto libre) queda como opción
explícita para entornos aislados, no como default.

Reglas de uso del contrato:
- Endpoints **estables** permitidos en rutas calientes: `session.*`, `permission.*`,
  `event.subscribe`, `project.*`, `model.*`, `vcs.*`, `worktree.*`.
- Endpoints `experimental/*` solo detrás de *feature detection* y con fallback documentado.
- La versión de OpenCode está pinneada y el contrato vive en el repo. Actualizarla es un
  cambio deliberado con diff de `docs/contracts/` en el mismo commit.

## Consecuencias

**Positivas**
- No construimos runtime de agentes: el esfuerzo va al plano de control, que es el diferencial.
- Un solo proceso compartido: menos memoria, arranque instantáneo, estado consistente.
- Coste real por sesión disponible vía `session.usage.updated`.
- Permisos y forms ya existen: la TUI y Telegram solo tienen que renderizarlos.

**Negativas / costes**
- Dependemos de la estabilidad de una API v2 (`0.0.1`, "experimental" en su descripción).
  Mitigación: contrato versionado + tests de contrato + trait de backend.
- Compartimos ciclo de vida con el servicio del usuario; una actualización suya puede rompernos.
- Si el daemon arranca el servicio, debe saber distinguir "lo arranqué yo" de "ya estaba".

**Riesgos mitigados**
- Autenticación: Basic `opencode:<password>`; el password se lee del archivo de servicio
  (0600) o del entorno, nunca se loguea ni se persiste en nuestra base.
- Caída del stream SSE: reconexión con `Last-Event-ID` + reconciliación REST por
  `session.message.list` / `session.diff` para no perder eventos.
- Servicio caído a mitad de tarea: el run queda `interrupted`, no `failed`, y es reintentable.

## Verificación

- `tests/contract.rs` valida que los endpoints usados existen en `docs/contracts/*.json`
  y que sus `operationId` no han cambiado (falla si el contrato se desvía).
- Test E2E opt-in (`JCS_E2E=1`): crear sesión en un repo temporal, pedir un cambio trivial,
  esperar `session.execution.succeeded`, leer el diff y el coste.
- `jcs doctor` reporta versión de OpenCode detectada vs versión soportada.
