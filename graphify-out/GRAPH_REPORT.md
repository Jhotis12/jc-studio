# Graph Report - jc-studio  (2026-11-21)

## Corpus Check
- 89 files · ~52,637 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 12 file(s) not represented in the graph (top: .toml 8, (none) 3, .service 1)

## Summary
- 1266 nodes · 2682 edges · 95 communities (62 shown, 33 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 79 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `dd468f21`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- src/store.rs
- OpenCodeClient
- Repo
- executor.rs
- commit.rs
- Client
- http.rs
- render.rs
- ApiError
- status.rs
- jobs
- event.rs
- sse.rs
- state.rs
- AppState
- tests/store.rs
- jcs-cli/src/main.rs
- commit.rs
- parser.rs
- Command
- pathbuf
- worktrees.rs
- JC Studio
- jcs-cli/src/client.rs
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
- runs.rs
- AGENTS.md
- loader.rs
- EventKind
- jcs-proto/src/lib.rs
- model.rs
- chaos.sh
- e2e.sh
- jcs-core/src/lib.rs
- check_contract.py
- Store
- jcs-policy/src/model.rs
- PolicyError
- jcs-policy/src/render.rs
- render_project.rs
- Endpoint
- jcs-agent/src/client.rs
- socket.rs
- jcs-agent/src/events.rs
- jcs-policy/src/lib.rs
- StatusResponse
- StoreError
- jcs-agent/src/lib.rs
- e2e-policy.sh
- ids.rs
- AgentEvent
- StatusMachine
- router
- jcs-daemon/src/main.rs
- CoreError
- Pendientes y riesgos aceptados
- ParserKind
- 0007 — Packs en TOML, composición determinista y bucle de reparación
- gate_views
- .gates
- ask
- core-git/instructions.md
- core-security/instructions.md
- postgres-schema/instructions.md
- rust-service/instructions.md
- testing-tdd/instructions.md
- ts-frontend/instructions.md

## God Nodes (most connected - your core abstractions)
1. `AppState` - 48 edges
2. `Client` - 34 edges
3. `Repo` - 32 edges
4. `Store` - 30 edges
5. `OpenCodeClient` - 29 edges
6. `StoreError` - 25 edges
7. `Pack` - 23 edges
8. `Project` - 22 edges
9. `builtin_packs()` - 20 edges
10. `NewEvent` - 19 edges

## Surprising Connections (you probably didn't know these)
- `run_task()` --calls--> `branch_name()`  [INFERRED]
  crates/jcs-daemon/src/executor.rs → crates/jcs-git/src/commit.rs
- `compile()` --calls--> `compose()`  [INFERRED]
  crates/jcs-policy/src/lib.rs → crates/jcs-policy/src/compose.rs
- `composition()` --calls--> `compose()`  [INFERRED]
  crates/jcs-policy/src/render.rs → crates/jcs-policy/src/compose.rs
- `rust_policy()` --calls--> `builtin_packs()`  [INFERRED]
  crates/jcs-policy/src/compose.rs → crates/jcs-policy/src/loader.rs
- `rust_policy()` --calls--> `resolve()`  [INFERRED]
  crates/jcs-policy/src/compose.rs → crates/jcs-policy/src/resolve.rs

## Import Cycles
- None detected.

## Communities (95 total, 33 thin omitted)

### Community 0 - "src/store.rs"
Cohesion: 0.11
Nodes (5): GoalRow, ProjectRow, decode_ts(), is_unique_violation(), TaskRow

### Community 1 - "OpenCodeClient"
Cohesion: 0.21
Nodes (4): into_list(), OpenCodeClient, unwrap_data(), wait_until_busy()

### Community 3 - "executor.rs"
Cohesion: 0.05
Nodes (44): Goal, Project, Task, block_for_input(), build_prompt(), cancel_pending_forms(), commit_message(), commit_messages_are_always_valid() (+36 more)

### Community 4 - "commit.rs"
Cohesion: 0.12
Nodes (13): builder_rejects_a_bad_summary(), builder_renders_header_and_body(), CommitKind, Chore, Ci, Docs, Feat, Fix (+5 more)

### Community 6 - "http.rs"
Cohesion: 0.10
Nodes (9): event_history(), events(), EventsQuery, file_state_label(), first_line(), HistoryQuery, LIST_LIMIT, protocol_guard() (+1 more)

### Community 7 - "render.rs"
Cohesion: 0.13
Nodes (19): duration(), durations_are_human_scaled(), plural(), plurals_read_naturally(), run_detail(), short_id(), short_ids_drop_the_prefix_and_bound_the_length(), status() (+11 more)

### Community 8 - "ApiError"
Cohesion: 0.10
Nodes (16): AgentError, Api, Decode, Discovery, Io, Json, Stream, Transport (+8 more)

### Community 9 - "status.rs"
Cohesion: 0.21
Nodes (4): any_goal(), any_run(), any_task(), status_enum!

### Community 11 - "jobs"
Cohesion: 0.08
Nodes (29): name, runs-on, steps, name, runs-on, steps, env, CARGO_TERM_COLOR (+21 more)

### Community 12 - "event.rs"
Cohesion: 0.13
Nodes (5): builder_sets_scopes_and_payload(), EVENT_SCHEMA_VERSION, event_kinds!, NewEvent, timestamps_encode_with_fixed_precision_utc()

### Community 13 - "sse.rs"
Cohesion: 0.21
Nodes (16): a_trailing_lone_cr_waits_for_the_next_chunk(), accepts_crlf_and_bare_cr_line_endings(), blank_lines_between_comment_only_blocks_do_not_dispatch(), captures_event_and_id_fields_when_present(), data_without_space_after_colon_is_valid(), decode_all(), decodes_a_minimal_data_only_event(), find_line_end() (+8 more)

### Community 14 - "state.rs"
Cohesion: 0.17
Nodes (4): Config, env_non_empty(), env_parse(), AgentHandle

### Community 15 - "AppState"
Cohesion: 0.28
Nodes (19): ApiResult, abort(), compile_policy(), gates(), health(), list_goals(), list_projects(), list_runs() (+11 more)

### Community 16 - "tests/store.rs"
Cohesion: 0.18
Nodes (13): an_empty_gate_set_is_not_a_pass(), backup_produces_a_readable_snapshot(), crashed_runs_become_interrupted_and_retryable(), deleting_a_project_cascades_state_but_preserves_the_audit_trail(), event_log_is_monotonic_and_resumable(), fixture(), goal_and_task_transitions_are_enforced_by_the_store(), orphaned_tasks_are_parked_as_blocked_not_failed() (+5 more)

### Community 19 - "jcs-cli/src/main.rs"
Cohesion: 0.15
Nodes (18): ask(), AskArgs, display_command(), doctor(), follow_run(), gates(), GatesCommand, List (+10 more)

### Community 20 - "commit.rs"
Cohesion: 0.21
Nodes (16): a_well_formed_message_passes(), branch_name(), branch_names_carry_the_goal_and_a_task_suffix(), missing_separator_is_rejected(), overlong_summaries_are_rejected(), scope_is_optional_and_may_contain_dashes(), slugify(), slugs_are_ascii_bounded_and_dash_separated() (+8 more)

### Community 22 - "parser.rs"
Cohesion: 0.06
Nodes (64): GateStatus, run_all(), coverage_threshold_only_makes_sense_with_the_coverage_parser(), empty_commands_are_rejected(), ensure_directory(), Finding, gate(), GateDef (+56 more)

### Community 23 - "Command"
Cohesion: 0.10
Nodes (20): Cli, Command, Abort, Ask, Doctor, Gates, Logs, Ping (+12 more)

### Community 24 - "pathbuf"
Cohesion: 0.29
Nodes (4): render_command(), Result, CommitOutcome, Worktree

### Community 25 - "worktrees.rs"
Cohesion: 0.23
Nodes (11): a_dirty_worktree_is_reported(), a_dirty_worktree_needs_force_to_be_discarded(), a_repository_without_commits_is_rejected(), a_worktree_isolates_changes_until_merge(), commit_and_branch_state_are_readable(), creating_a_worktree_over_an_existing_path_is_refused(), fixture(), git() (+3 more)

### Community 26 - "JC Studio"
Cohesion: 0.17
Nodes (12): Arquitectura, Cómo termina un run, Desarrollo, Estado, Instalación, JC Studio, Licencia, Política del proyecto (+4 more)

### Community 27 - "jcs-cli/src/client.rs"
Cohesion: 0.11
Nodes (5): EventStream, prepare_socket(), restrict_permissions(), serve(), serve_connection()

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
Cohesion: 0.33
Nodes (6): 0004 — Política como código con doble capa: instrucciones + gates ejecutables, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 35 - "0005 — Un worktree por tarea y git vía CLI"
Cohesion: 0.29
Nodes (6): 0005 — Un worktree por tarea y git vía CLI, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 36 - "0006 — TUI ratatui y streaming NDJSON sobre el socket (no SSE para el cliente)"
Cohesion: 0.29
Nodes (6): 0006 — TUI ratatui y streaming NDJSON sobre el socket (no SSE para el cliente), Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 37 - "jcs-daemon"
Cohesion: 0.39
Nodes (9): jcs-agent, jcs-cli, jcs-core, jcs-daemon, jcs-gates, jcs-git, jcs-policy, jcs-proto (+1 more)

### Community 45 - "loader.rs"
Cohesion: 0.15
Nodes (24): a_missing_instruction_file_is_an_error(), built_in_packs_load_and_validate(), BUILTIN, builtin_packs(), compute_hash(), disk_packs_override_built_ins_by_id(), Embedded, instruction_files_cannot_escape_the_pack_directory() (+16 more)

### Community 49 - "EventKind"
Cohesion: 0.18
Nodes (8): EventKind, Result, goal_columns!, GateResult, GateResultRow, run_columns!, append_event(), task_columns!

### Community 50 - "jcs-proto/src/lib.rs"
Cohesion: 0.09
Nodes (25): add_project(), project_view(), AbortRequest, AddProjectRequest, AgentHealth, ErrorBody, errors_carry_a_code_and_a_message(), EventDto (+17 more)

### Community 51 - "model.rs"
Cohesion: 0.18
Nodes (6): duration_requires_an_end(), only_process_failures_are_retryable(), Run, sample_run(), RunStatus, RunRow

### Community 52 - "chaos.sh"
Cohesion: 0.24
Nodes (10): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, chaos.sh script, start_daemon(), step() (+2 more)

### Community 53 - "e2e.sh"
Cohesion: 0.25
Nodes (9): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e.sh script, step(), wait_for_daemon() (+1 more)

### Community 56 - "Store"
Cohesion: 0.15
Nodes (9): Event, count_events(), events_for_task(), events_since(), latest_event_ts(), latest_seq(), recent_events(), row_to_event() (+1 more)

### Community 57 - "jcs-policy/src/model.rs"
Cohesion: 0.17
Nodes (15): compose(), gate_sources_are_recorded(), instructions_follow_pack_order_and_keep_provenance(), later_packs_override_earlier_rules(), Overrides, project_overrides_win_over_every_pack(), rules_and_gates_can_be_disabled(), rust_policy() (+7 more)

### Community 58 - "PolicyError"
Cohesion: 0.12
Nodes (17): PolicyError, Cycle, InvalidGate, InvalidPack, Io, MissingPack, PathEscape, Refused (+9 more)

### Community 59 - "jcs-policy/src/render.rs"
Cohesion: 0.21
Nodes (20): sha256_hex(), agents_md_carries_the_marker_packs_and_rule_provenance(), changing_a_rule_changes_the_rendered_output(), composition(), content_of(), GateDocument, gates_toml_round_trips_to_the_same_gate_set(), GatesDocument (+12 more)

### Community 60 - "render_project.rs"
Cohesion: 0.29
Nodes (16): apply(), compile(), Compiled, a_hand_written_agents_md_is_never_silently_replaced(), a_project_without_a_policy_file_uses_the_defaults(), an_unknown_pack_fails_with_an_actionable_message(), apply_creates_then_reports_unchanged(), check_passes_right_after_apply_and_fails_after_a_hand_edit() (+8 more)

### Community 61 - "Endpoint"
Cohesion: 0.16
Nodes (7): base_url_loses_trailing_slashes(), debug_never_prints_the_password(), Endpoint, service_password(), service_url(), opencode_config_dir(), Result

### Community 62 - "jcs-agent/src/client.rs"
Cohesion: 0.14
Nodes (8): api_errors_surface_the_server_message(), data_envelope_is_unwrapped_but_bare_payloads_survive(), EventStream, extract_error_message(), lists_are_extracted_from_both_shapes(), PromptAck, REQUEST_TIMEOUT, truncate()

### Community 63 - "socket.rs"
Cohesion: 0.21
Nodes (8): a_second_daemon_refuses_to_steal_a_live_socket(), a_stale_socket_from_a_killed_daemon_is_replaced(), Daemon, get_json(), spawn_daemon(), the_daemon_serves_health_and_history_over_a_socket(), wait_for_socket(), a_second_store_cannot_open_the_same_database()

### Community 64 - "jcs-agent/src/events.rs"
Cohesion: 0.25
Nodes (11): decodes_the_real_captured_execution_event(), decodes_the_real_captured_usage_event(), extracts_text_deltas_only_from_delta_events(), frame(), non_json_payloads_are_an_error_with_context(), tool_events_are_read_defensively(), ToolInvocation, truncate() (+3 more)

### Community 65 - "jcs-policy/src/lib.rs"
Cohesion: 0.15
Nodes (13): ApplyReport, FileReport, FileState, Created, Drifted, Missing, Refused, Unchanged (+5 more)

### Community 66 - "StatusResponse"
Cohesion: 0.16
Nodes (11): goal_view(), run_view(), run_views_expose_cost_and_tokens(), task_view(), GateView, GoalView, RunDetailResponse, RunView (+3 more)

### Community 67 - "StoreError"
Cohesion: 0.13
Nodes (10): StoreError, Conflict, Corrupt, Db, Domain, Io, Json, Locked (+2 more)

### Community 68 - "jcs-agent/src/lib.rs"
Cohesion: 0.18
Nodes (7): AgentSession, CreateSession, model_reference_parses_and_roundtrips(), ModelRef, ServerInfo, SessionState, SessionTime

### Community 69 - "e2e-policy.sh"
Cohesion: 0.18
Nodes (11): fail(), JCS_DB, JCS_DEFAULT_PACKS, JCS_MAX_CONCURRENT, JCS_MAX_REPAIR_CYCLES, JCS_PACKS_DIR, JCS_SOCKET, RUST_LOG (+3 more)

### Community 71 - "AgentEvent"
Cohesion: 0.29
Nodes (5): AgentEvent, ExecutionOutcome, Failed, Interrupted, Succeeded

### Community 72 - "StatusMachine"
Cohesion: 0.24
Nodes (3): failed_task_can_only_retry_or_cancel(), StatusMachine, transition_error_names_the_entity_and_both_states()

### Community 73 - "router"
Cohesion: 0.40
Nodes (9): a_directory_that_is_not_a_repository_is_rejected(), a_registered_repository_appears_in_the_list(), a_relative_project_path_is_rejected_with_a_reason(), asking_for_an_unknown_project_suggests_the_next_step(), health_reports_the_protocol_version(), record_lifecycle(), router(), send() (+1 more)

### Community 74 - "jcs-daemon/src/main.rs"
Cohesion: 0.31
Nodes (5): Args, init_tracing(), main(), restrict_dir(), shutdown_signal()

### Community 75 - "CoreError"
Cohesion: 0.29
Nodes (5): CoreError, EmptyField, InvalidId, InvalidTransition, InvalidValue

### Community 76 - "Pendientes y riesgos aceptados"
Cohesion: 0.29
Nodes (5): Decisiones diferidas (no son deuda, son alcance), Limitaciones conocidas, Pendientes y riesgos aceptados, Riesgos aceptados, RUSTSEC-2026-0459 — SQLite empaquetado en `sqlx` 0.9

### Community 77 - "ParserKind"
Cohesion: 0.33
Nodes (6): ParserKind, Cargo, Coverage, ExitCode, Generic, Node

### Community 78 - "0007 — Packs en TOML, composición determinista y bucle de reparación"
Cohesion: 0.33
Nodes (6): 0007 — Packs en TOML, composición determinista y bucle de reparación, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 79 - "gate_views"
Cohesion: 0.40
Nodes (3): gate_views(), parser_label(), PolicyGateView

### Community 81 - "ask"
Cohesion: 0.40
Nodes (5): fail_task(), spawn(), ask(), AskRequest, AskResponse

## Knowledge Gaps
- **102 isolated node(s):** `Project`, `Ask`, `Status`, `Logs`, `Runs` (+97 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 402 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **33 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Client` connect `Client` to `.gates`, `jcs-cli/src/main.rs`, `jcs-cli/src/client.rs`?**
  _High betweenness centrality (0.057) - this node is a cross-community bridge._
- **What connects `Project`, `Ask`, `Status` to the rest of the system?**
  _102 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `src/store.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1111111111111111 - nodes in this community are weakly interconnected._
- **Why does `run` connect `jobs` to `parser.rs`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **Should `executor.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.051685393258426963 - nodes in this community are weakly interconnected._
- **Why does `AppState` connect `AppState` to `jcs-agent/src/events.rs`, `executor.rs`, `http.rs`, `router`, `jcs-daemon/src/main.rs`, `loader.rs`, `state.rs`, `ask`, `jcs-proto/src/lib.rs`, `.fmt`, `Store`, `jcs-cli/src/client.rs`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **Should `commit.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.12380952380952381 - nodes in this community are weakly interconnected._