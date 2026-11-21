# Graph Report - jc-studio  (2026-11-21)

## Corpus Check
- 63 files · ~37,448 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 6 file(s) not represented in the graph (top: (none) 3, .toml 2, .service 1)

## Summary
- 917 nodes · 1894 edges · 49 communities (29 shown, 20 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 40 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `ae90c4a6`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- .pool
- jcs-agent/src/client.rs
- Repo
- executor.rs
- commit.rs
- Client
- http.rs
- render.rs
- ApiError
- status.rs
- jobs
- Store
- jcs-agent/src/events.rs
- sse.rs
- tests/store.rs
- jcs-cli/src/main.rs
- commit.rs
- .get
- Command
- pathbuf
- worktrees.rs
- JC Studio
- Contribuir a JC Studio
- Contrato con OpenCode
- 0001 — Daemon `jcsd` + cliente fino por Unix socket
- adr/README.md
- 0002 — OpenCode v2 HTTP API como backend de agentes (modo attach)
- 0003 — SQLite + sqlx con event log append-only
- 0004 — Política como código con doble capa: instrucciones + gates ejecutables
- 0005 — Un worktree por tarea y git vía CLI
- 0006 — TUI ratatui y streaming NDJSON sobre el socket (no SSE para el cliente)
- jcs-daemon
- AGENTS.md

## God Nodes (most connected - your core abstractions)
1. `AppState` - 39 edges
2. `Repo` - 32 edges
3. `Store` - 30 edges
4. `OpenCodeClient` - 28 edges
5. `Client` - 27 edges
6. `StoreError` - 25 edges
7. `NewEvent` - 19 edges
8. `Project` - 19 edges
9. `Task` - 18 edges
10. `ApiError` - 18 edges

## Surprising Connections (you probably didn't know these)
- `run_task()` --calls--> `branch_name()`  [INFERRED]
  crates/jcs-daemon/src/executor.rs → crates/jcs-git/src/commit.rs
- `status()` --calls--> `status()`  [EXTRACTED]
  crates/jcs-cli/src/main.rs → crates/jcs-cli/src/render.rs
- `doctor()` --calls--> `doctor_check()`  [EXTRACTED]
  crates/jcs-cli/src/main.rs → crates/jcs-cli/src/render.rs
- `record_lifecycle()` --references--> `EventKind`  [EXTRACTED]
  crates/jcs-daemon/src/http.rs → crates/jcs-core/src/event.rs
- `finalize_success()` --references--> `Project`  [EXTRACTED]
  crates/jcs-daemon/src/executor.rs → crates/jcs-core/src/model.rs

## Import Cycles
- None detected.

## Communities (49 total, 20 thin omitted)

### Community 0 - ".pool"
Cohesion: 0.05
Nodes (26): duration_requires_an_end(), Goal, only_process_failures_are_retryable(), Project, Run, sample_run(), Task, GoalStatus (+18 more)

### Community 1 - "jcs-agent/src/client.rs"
Cohesion: 0.07
Nodes (23): AgentSession, api_errors_surface_the_server_message(), CreateSession, data_envelope_is_unwrapped_but_bare_payloads_survive(), EventStream, extract_error_message(), into_list(), lists_are_extracted_from_both_shapes() (+15 more)

### Community 3 - "executor.rs"
Cohesion: 0.05
Nodes (45): Config, env_non_empty(), env_parse(), block_for_input(), build_prompt(), cancel_pending_forms(), commit_message(), commit_messages_are_always_valid() (+37 more)

### Community 4 - "commit.rs"
Cohesion: 0.12
Nodes (13): builder_rejects_a_bad_summary(), builder_renders_header_and_body(), CommitKind, Chore, Ci, Docs, Feat, Fix (+5 more)

### Community 6 - "http.rs"
Cohesion: 0.05
Nodes (56): ApiResult, a_directory_that_is_not_a_repository_is_rejected(), a_registered_repository_appears_in_the_list(), a_relative_project_path_is_rejected_with_a_reason(), abort(), add_project(), asking_for_an_unknown_project_suggests_the_next_step(), event_history() (+48 more)

### Community 7 - "render.rs"
Cohesion: 0.15
Nodes (19): doctor_check(), duration(), durations_are_human_scaled(), plural(), plurals_read_naturally(), short_id(), short_ids_drop_the_prefix_and_bound_the_length(), status() (+11 more)

### Community 8 - "ApiError"
Cohesion: 0.06
Nodes (26): AgentError, Api, Decode, Discovery, Io, Json, Stream, Transport (+18 more)

### Community 9 - "status.rs"
Cohesion: 0.06
Nodes (15): CoreError, EmptyField, InvalidId, InvalidTransition, InvalidValue, id_type!, any_goal(), any_run() (+7 more)

### Community 11 - "jobs"
Cohesion: 0.08
Nodes (29): name, runs-on, steps, name, runs-on, steps, env, CARGO_TERM_COLOR (+21 more)

### Community 12 - "Store"
Cohesion: 0.05
Nodes (18): builder_sets_scopes_and_payload(), Event, EVENT_SCHEMA_VERSION, EventKind, event_kinds!, NewEvent, timestamps_encode_with_fixed_precision_utc(), Result (+10 more)

### Community 13 - "jcs-agent/src/events.rs"
Cohesion: 0.16
Nodes (15): AgentEvent, decodes_the_real_captured_execution_event(), decodes_the_real_captured_usage_event(), ExecutionOutcome, Failed, Interrupted, Succeeded, extracts_text_deltas_only_from_delta_events() (+7 more)

### Community 14 - "sse.rs"
Cohesion: 0.21
Nodes (16): a_trailing_lone_cr_waits_for_the_next_chunk(), accepts_crlf_and_bare_cr_line_endings(), blank_lines_between_comment_only_blocks_do_not_dispatch(), captures_event_and_id_fields_when_present(), data_without_space_after_colon_is_valid(), decode_all(), decodes_a_minimal_data_only_event(), find_line_end() (+8 more)

### Community 16 - "tests/store.rs"
Cohesion: 0.06
Nodes (25): prepare_socket(), restrict_permissions(), serve(), serve_connection(), a_second_daemon_refuses_to_steal_a_live_socket(), a_stale_socket_from_a_killed_daemon_is_replaced(), Daemon, get_json() (+17 more)

### Community 19 - "jcs-cli/src/main.rs"
Cohesion: 0.18
Nodes (13): ask(), AskArgs, doctor(), follow_run(), logs(), LogsArgs, main(), project() (+5 more)

### Community 20 - "commit.rs"
Cohesion: 0.20
Nodes (17): a_well_formed_message_passes(), branch_name(), branch_names_carry_the_goal_and_a_task_suffix(), missing_separator_is_rejected(), overlong_summaries_are_rejected(), scope_is_optional_and_may_contain_dashes(), slugify(), slugs_are_ascii_bounded_and_dash_separated() (+9 more)

### Community 23 - "Command"
Cohesion: 0.15
Nodes (13): Cli, Command, Abort, Ask, Doctor, Logs, Project, Runs (+5 more)

### Community 24 - "pathbuf"
Cohesion: 0.22
Nodes (3): Result, render_command(), Result

### Community 25 - "worktrees.rs"
Cohesion: 0.23
Nodes (11): a_dirty_worktree_is_reported(), a_dirty_worktree_needs_force_to_be_discarded(), a_repository_without_commits_is_rejected(), a_worktree_isolates_changes_until_merge(), commit_and_branch_state_are_readable(), creating_a_worktree_over_an_existing_path_is_refused(), fixture(), git() (+3 more)

### Community 26 - "JC Studio"
Cohesion: 0.18
Nodes (11): Arquitectura, Cómo termina un run, Desarrollo, Estado, Instalación, JC Studio, Licencia, Principios (+3 more)

### Community 28 - "Contribuir a JC Studio"
Cohesion: 0.22
Nodes (8): Checklist de PR, Contribuir a JC Studio, Convenciones de código, Flujo de trabajo, Idioma, Mensajes de commit (Conventional Commits), Seguridad, Tests

### Community 29 - "Contrato con OpenCode"
Cohesion: 0.22
Nodes (8): Al actualizar OpenCode, Ciclo de vida observado (modelo gratuito, ~5 s), Contrato con OpenCode, Descubrimiento del servicio, Endpoints usados, Formas de respuesta (inconsistentes, a propósito), Permisos, Stream de eventos: `GET /api/event`

### Community 30 - "0001 — Daemon `jcsd` + cliente fino por Unix socket"
Cohesion: 0.29
Nodes (6): 0001 — Daemon `jcsd` + cliente fino por Unix socket, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 31 - "adr/README.md"
Cohesion: 0.29
Nodes (3): ADRs — JC Studio, Cómo añadir una ADR, Regla de oro

### Community 32 - "0002 — OpenCode v2 HTTP API como backend de agentes (modo attach)"
Cohesion: 0.29
Nodes (7): 0002 — OpenCode v2 HTTP API como backend de agentes (modo attach), Alternativas consideradas, Consecuencias, Contexto, Decisión, Enmienda (2026-11-21): reconciliación y formularios, Verificación

### Community 33 - "0003 — SQLite + sqlx con event log append-only"
Cohesion: 0.25
Nodes (8): 0003 — SQLite + sqlx con event log append-only, Alternativas consideradas, Consecuencias, Contexto, Decisión, Enmienda (2026-11-21): arranque sobre estado sucio, Enmienda (2026-11-21): de `chrono` a `jiff`, y el estado de SQLite, Verificación

### Community 34 - "0004 — Política como código con doble capa: instrucciones + gates ejecutables"
Cohesion: 0.29
Nodes (6): 0004 — Política como código con doble capa: instrucciones + gates ejecutables, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 35 - "0005 — Un worktree por tarea y git vía CLI"
Cohesion: 0.29
Nodes (6): 0005 — Un worktree por tarea y git vía CLI, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 36 - "0006 — TUI ratatui y streaming NDJSON sobre el socket (no SSE para el cliente)"
Cohesion: 0.29
Nodes (6): 0006 — TUI ratatui y streaming NDJSON sobre el socket (no SSE para el cliente), Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 37 - "jcs-daemon"
Cohesion: 0.43
Nodes (7): jcs-agent, jcs-cli, jcs-core, jcs-daemon, jcs-git, jcs-proto, jcs-store

## Knowledge Gaps
- **51 isolated node(s):** `TimedOut`, `Strict`, `Auto`, `Failed`, `Interrupted` (+46 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 289 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **20 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `EventKind` connect `Store` to `.pool`, `executor.rs`, `http.rs`, `render.rs`, `tests/store.rs`, `jcs-cli/src/main.rs`?**
  _High betweenness centrality (0.055) - this node is a cross-community bridge._
- **What connects `TimedOut`, `Strict`, `Auto` to the rest of the system?**
  _51 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `.pool` be split into smaller, more focused modules?**
  _Cohesion score 0.050101010101010104 - nodes in this community are weakly interconnected._
- **Why does `Repo` connect `Repo` to `pathbuf`, `worktrees.rs`, `commit.rs`?**
  _High betweenness centrality (0.047) - this node is a cross-community bridge._
- **Should `jcs-agent/src/client.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06538461538461539 - nodes in this community are weakly interconnected._
- **Why does `Client` connect `Client` to `.add_project`, `tests/store.rs`, `jcs-cli/src/main.rs`, `.get`, `.event_history`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **Should `Repo` be split into smaller, more focused modules?**
  _Cohesion score 0.14814814814814814 - nodes in this community are weakly interconnected._