# Graph Report - jc-studio  (2026-11-21)

## Corpus Check
- 62 files · ~36,626 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 6 file(s) not represented in the graph (top: (none) 3, .toml 2, .service 1)

## Summary
- 888 nodes · 1881 edges · 40 communities (37 shown, 3 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 37 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `0605075c`
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
- jcs-daemon/src/main.rs
- ApiError
- status.rs
- jcs-proto/src/lib.rs
- jobs
- NewEvent
- jcs-agent/src/events.rs
- sse.rs
- AppState
- jcs-cli/src/client.rs
- src/store.rs
- Store
- jcs-cli/src/main.rs
- runs.rs
- tests/store.rs
- StoreError
- Command
- EventKind
- render.rs
- JC Studio
- jcs-core/src/lib.rs
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
- check_contract.py
- AGENTS.md

## God Nodes (most connected - your core abstractions)
1. `AppState` - 39 edges
2. `Repo` - 33 edges
3. `Store` - 30 edges
4. `OpenCodeClient` - 28 edges
5. `Client` - 27 edges
6. `NewEvent` - 25 edges
7. `StoreError` - 25 edges
8. `Event` - 20 edges
9. `Project` - 20 edges
10. `Task` - 19 edges

## Surprising Connections (you probably didn't know these)
- `run_task()` --calls--> `branch_name()`  [INFERRED]
  crates/jcs-daemon/src/executor.rs → crates/jcs-git/src/commit.rs
- `latest_event_ts()` --calls--> `decode_ts()`  [INFERRED]
  crates/jcs-store/src/events.rs → crates/jcs-store/src/store.rs
- `jcs-daemon` --depends_on--> `jcs-agent`  [EXTRACTED]
  crates/jcs-daemon/Cargo.toml → crates/jcs-agent/Cargo.toml
- `cancel_pending_forms()` --references--> `OpenCodeClient`  [EXTRACTED]
  crates/jcs-daemon/src/executor.rs → crates/jcs-agent/src/client.rs
- `AgentHandle` --references--> `OpenCodeClient`  [EXTRACTED]
  crates/jcs-daemon/src/state.rs → crates/jcs-agent/src/client.rs

## Import Cycles
- None detected.

## Communities (40 total, 3 thin omitted)

### Community 0 - ".pool"
Cohesion: 0.07
Nodes (17): duration_requires_an_end(), Goal, only_process_failures_are_retryable(), Project, Run, sample_run(), Task, RunStatus (+9 more)

### Community 1 - "jcs-agent/src/client.rs"
Cohesion: 0.06
Nodes (24): AgentSession, api_errors_surface_the_server_message(), CreateSession, data_envelope_is_unwrapped_but_bare_payloads_survive(), EventStream, extract_error_message(), into_list(), lists_are_extracted_from_both_shapes() (+16 more)

### Community 2 - "Repo"
Cohesion: 0.08
Nodes (15): render_command(), CommitOutcome, Repo, Worktree, a_dirty_worktree_is_reported(), a_dirty_worktree_needs_force_to_be_discarded(), a_repository_without_commits_is_rejected(), a_worktree_isolates_changes_until_merge() (+7 more)

### Community 3 - "executor.rs"
Cohesion: 0.10
Nodes (34): block_for_input(), build_prompt(), cancel_pending_forms(), commit_message(), commit_messages_are_always_valid(), decide(), describe_form(), fail_task() (+26 more)

### Community 4 - "commit.rs"
Cohesion: 0.09
Nodes (30): a_well_formed_message_passes(), branch_name(), branch_names_carry_the_goal_and_a_task_suffix(), builder_rejects_a_bad_summary(), builder_renders_header_and_body(), CommitKind, Chore, Ci (+22 more)

### Community 5 - "Client"
Cohesion: 0.09
Nodes (3): Client, decode(), EventStream

### Community 6 - "http.rs"
Cohesion: 0.10
Nodes (18): a_directory_that_is_not_a_repository_is_rejected(), a_registered_repository_appears_in_the_list(), a_relative_project_path_is_rejected_with_a_reason(), asking_for_an_unknown_project_suggests_the_next_step(), events(), EventsQuery, first_line(), health_reports_the_protocol_version() (+10 more)

### Community 7 - "jcs-daemon/src/main.rs"
Cohesion: 0.09
Nodes (19): Config, env_non_empty(), env_parse(), Args, init_tracing(), main(), restrict_dir(), shutdown_signal() (+11 more)

### Community 8 - "ApiError"
Cohesion: 0.10
Nodes (16): AgentError, Api, Decode, Discovery, Io, Json, Stream, Transport (+8 more)

### Community 9 - "status.rs"
Cohesion: 0.07
Nodes (10): CoreError, EmptyField, InvalidId, InvalidTransition, InvalidValue, id_type!, any_goal(), any_run() (+2 more)

### Community 10 - "jcs-proto/src/lib.rs"
Cohesion: 0.10
Nodes (24): goal_view(), project_view(), task_view(), AbortRequest, AddProjectRequest, AgentHealth, AskResponse, ErrorBody (+16 more)

### Community 11 - "jobs"
Cohesion: 0.08
Nodes (29): name, runs-on, steps, name, runs-on, steps, env, CARGO_TERM_COLOR (+21 more)

### Community 12 - "NewEvent"
Cohesion: 0.15
Nodes (5): builder_sets_scopes_and_payload(), Event, event_kinds!, NewEvent, timestamps_encode_with_fixed_precision_utc()

### Community 13 - "jcs-agent/src/events.rs"
Cohesion: 0.16
Nodes (15): AgentEvent, decodes_the_real_captured_execution_event(), decodes_the_real_captured_usage_event(), ExecutionOutcome, Failed, Interrupted, Succeeded, extracts_text_deltas_only_from_delta_events() (+7 more)

### Community 14 - "sse.rs"
Cohesion: 0.21
Nodes (16): a_trailing_lone_cr_waits_for_the_next_chunk(), accepts_crlf_and_bare_cr_line_endings(), blank_lines_between_comment_only_blocks_do_not_dispatch(), captures_event_and_id_fields_when_present(), data_without_space_after_colon_is_valid(), decode_all(), decodes_a_minimal_data_only_event(), find_line_end() (+8 more)

### Community 15 - "AppState"
Cohesion: 0.24
Nodes (14): ApiResult, abort(), add_project(), ask(), event_history(), health(), list_goals(), list_projects() (+6 more)

### Community 16 - "jcs-cli/src/client.rs"
Cohesion: 0.12
Nodes (4): prepare_socket(), restrict_permissions(), serve(), serve_connection()

### Community 17 - "src/store.rs"
Cohesion: 0.10
Nodes (5): row_to_event(), GoalRow, decode_ts(), is_unique_violation(), TaskRow

### Community 18 - "Store"
Cohesion: 0.17
Nodes (7): count_events(), events_for_task(), events_since(), latest_event_ts(), latest_seq(), recent_events(), Store

### Community 19 - "jcs-cli/src/main.rs"
Cohesion: 0.18
Nodes (13): ask(), AskArgs, doctor(), follow_run(), logs(), LogsArgs, main(), project() (+5 more)

### Community 20 - "runs.rs"
Cohesion: 0.15
Nodes (8): failed_task_can_only_retry_or_cancel(), GateStatus, StatusMachine, transition_error_names_the_entity_and_both_states(), GateResult, GateResultRow, RUN_COLUMNS, RunRow

### Community 21 - "tests/store.rs"
Cohesion: 0.19
Nodes (13): an_empty_gate_set_is_not_a_pass(), backup_produces_a_readable_snapshot(), crashed_runs_become_interrupted_and_retryable(), deleting_a_project_cascades_state_but_preserves_the_audit_trail(), event_log_is_monotonic_and_resumable(), fixture(), goal_and_task_transitions_are_enforced_by_the_store(), orphaned_tasks_are_parked_as_blocked_not_failed() (+5 more)

### Community 22 - "StoreError"
Cohesion: 0.13
Nodes (10): StoreError, Conflict, Corrupt, Db, Domain, Io, Json, Locked (+2 more)

### Community 23 - "Command"
Cohesion: 0.15
Nodes (13): Cli, Command, Abort, Ask, Doctor, Logs, Project, Runs (+5 more)

### Community 24 - "EventKind"
Cohesion: 0.23
Nodes (5): EventKind, Result, GOAL_COLUMNS, ProjectRow, TASK_COLUMNS

### Community 25 - "render.rs"
Cohesion: 0.31
Nodes (9): doctor_check(), duration(), durations_are_human_scaled(), plural(), plurals_read_naturally(), short_id(), short_ids_drop_the_prefix_and_bound_the_length(), status() (+1 more)

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
Cohesion: 0.29
Nodes (7): 0003 — SQLite + sqlx con event log append-only, Alternativas consideradas, Consecuencias, Contexto, Decisión, Enmienda (2026-11-21): arranque sobre estado sucio, Verificación

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
- **51 isolated node(s):** `jcs-agent`, `Discovery`, `Transport`, `Api`, `Decode` (+46 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 273 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **3 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Repo` connect `Repo` to `executor.rs`, `commit.rs`?**
  _High betweenness centrality (0.059) - this node is a cross-community bridge._
- **What connects `jcs-agent`, `Discovery`, `Transport` to the rest of the system?**
  _51 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `.pool` be split into smaller, more focused modules?**
  _Cohesion score 0.06518987341772152 - nodes in this community are weakly interconnected._
- **Why does `EventKind` connect `EventKind` to `executor.rs`, `http.rs`, `jcs-daemon/src/main.rs`, `NewEvent`, `Store`, `jcs-cli/src/main.rs`, `runs.rs`, `tests/store.rs`, `render.rs`, `jcs-core/src/lib.rs`?**
  _High betweenness centrality (0.049) - this node is a cross-community bridge._
- **Should `jcs-agent/src/client.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05875251509054326 - nodes in this community are weakly interconnected._
- **Why does `OpenCodeClient` connect `jcs-agent/src/client.rs` to `executor.rs`, `jcs-daemon/src/main.rs`?**
  _High betweenness centrality (0.047) - this node is a cross-community bridge._
- **Should `Repo` be split into smaller, more focused modules?**
  _Cohesion score 0.07764705882352942 - nodes in this community are weakly interconnected._