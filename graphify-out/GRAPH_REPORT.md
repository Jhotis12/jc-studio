# Graph Report - jc-studio  (2026-11-21)

## Corpus Check
- 106 files · ~68,871 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 12 file(s) not represented in the graph (top: .toml 8, (none) 3, .service 1)

## Summary
- 1594 nodes · 3524 edges · 125 communities (79 shown, 46 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 131 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `322d26e2`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- src/store.rs
- OpenCodeClient
- Repo
- Task
- commit.rs
- Client
- http.rs
- render.rs
- ApiError
- status.rs
- jobs
- event.rs
- sse.rs
- paths.rs
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
- AGENTS.md
- plans.rs
- loader.rs
- Result
- jcs-proto/src/lib.rs
- path
- chaos.sh
- e2e.sh
- mod.rs
- check_contract.py
- Store
- jcs-policy/src/model.rs
- PolicyError
- jcs-policy/src/render.rs
- render_project.rs
- Endpoint
- jcs-agent/src/client.rs
- Project
- jcs-agent/src/events.rs
- jcs-policy/src/lib.rs
- runner.rs
- StoreError
- ui.rs
- e2e-policy.sh
- jcs-core/src/model.rs
- AgentEvent
- .pool
- app.rs
- leases.rs
- jcs-gates/src/model.rs
- Pendientes y riesgos aceptados
- App
- 0007 — Packs en TOML, composición determinista y bucle de reparación
- draw
- .gates
- ask
- executor.rs
- core-git/instructions.md
- core-security/instructions.md
- postgres-schema/instructions.md
- rust-service/instructions.md
- testing-tdd/instructions.md
- ts-frontend/instructions.md
- GoalView
- Store
- PlanError
- gates
- plan.rs
- ids.rs
- Store
- Store
- e2e-merge.sh
- e2e-plan.sh
- ProjectView
- state.rs
- CoreError
- TaskRole
- .transition
- Plan
- ParserKind
- 0008 — Orquestador durable: plan, DAG, leases y rama de integración
- ProjectCommand
- Focus
- Severity

## God Nodes (most connected - your core abstractions)
1. `AppState` - 72 edges
2. `Client` - 45 edges
3. `Repo` - 40 edges
4. `Project` - 39 edges
5. `App` - 38 edges
6. `Store` - 35 edges
7. `OpenCodeClient` - 33 edges
8. `Goal` - 31 edges
9. `Task` - 31 edges
10. `NewEvent` - 29 edges

## Surprising Connections (you probably didn't know these)
- `run_task()` --calls--> `branch_name()`  [INFERRED]
  crates/jcs-daemon/src/executor.rs → crates/jcs-git/src/commit.rs
- `packs()` --references--> `Pack`  [INFERRED]
  crates/jcs-policy/tests/render_project.rs → crates/jcs-policy/src/model.rs
- `rust_policy()` --calls--> `builtin_packs()`  [INFERRED]
  crates/jcs-policy/src/compose.rs → crates/jcs-policy/src/loader.rs
- `composition()` --calls--> `builtin_packs()`  [INFERRED]
  crates/jcs-policy/src/render.rs → crates/jcs-policy/src/loader.rs
- `packs()` --calls--> `builtin_packs()`  [INFERRED]
  crates/jcs-policy/tests/render_project.rs → crates/jcs-policy/src/loader.rs

## Import Cycles
- None detected.

## Communities (125 total, 46 thin omitted)

### Community 0 - "src/store.rs"
Cohesion: 0.15
Nodes (4): append_event(), encode_ts(), insert_event(), is_unique_violation()

### Community 1 - "OpenCodeClient"
Cohesion: 0.21
Nodes (3): into_list(), OpenCodeClient, unwrap_data()

### Community 3 - "Task"
Cohesion: 0.17
Nodes (4): Task, TaskStatus, Store, TaskRow

### Community 4 - "commit.rs"
Cohesion: 0.12
Nodes (13): builder_rejects_a_bad_summary(), builder_renders_header_and_body(), CommitKind, Chore, Ci, Docs, Feat, Fix (+5 more)

### Community 6 - "http.rs"
Cohesion: 0.12
Nodes (13): a_directory_that_is_not_a_repository_is_rejected(), a_registered_repository_appears_in_the_list(), a_relative_project_path_is_rejected_with_a_reason(), asking_for_an_unknown_project_suggests_the_next_step(), events(), EventsQuery, health_reports_the_protocol_version(), LIST_LIMIT (+5 more)

### Community 7 - "render.rs"
Cohesion: 0.21
Nodes (13): follow_goal(), doctor_check(), duration(), durations_are_human_scaled(), event(), goal_detail(), plural(), plurals_read_naturally() (+5 more)

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
Cohesion: 0.11
Nodes (8): builder_sets_scopes_and_payload(), Event, EVENT_SCHEMA_VERSION, EventKind, event_kinds!, NewEvent, timestamps_encode_with_fixed_precision_utc(), MAX_PLAN_TASKS

### Community 13 - "sse.rs"
Cohesion: 0.21
Nodes (16): a_trailing_lone_cr_waits_for_the_next_chunk(), accepts_crlf_and_bare_cr_line_endings(), blank_lines_between_comment_only_blocks_do_not_dispatch(), captures_event_and_id_fields_when_present(), data_without_space_after_colon_is_valid(), decode_all(), decodes_a_minimal_data_only_event(), find_line_end() (+8 more)

### Community 14 - "paths.rs"
Cohesion: 0.18
Nodes (13): Config, env_non_empty(), env_parse(), SOCKET_NAME, data_dir(), database_lives_under_the_data_directory(), database_path(), log_dir() (+5 more)

### Community 15 - "AppState"
Cohesion: 0.20
Nodes (16): ApiResult, abort(), event_history(), health(), HistoryQuery, list_goals(), list_projects(), list_runs() (+8 more)

### Community 16 - "tests/store.rs"
Cohesion: 0.21
Nodes (13): an_empty_gate_set_is_not_a_pass(), backup_produces_a_readable_snapshot(), crashed_runs_become_interrupted_and_retryable(), deleting_a_project_cascades_state_but_preserves_the_audit_trail(), event_log_is_monotonic_and_resumable(), fixture(), goal_and_task_transitions_are_enforced_by_the_store(), orphaned_tasks_are_parked_as_blocked_not_failed() (+5 more)

### Community 19 - "jcs-cli/src/main.rs"
Cohesion: 0.18
Nodes (18): Client, ask(), AskArgs, Cli, display_command(), doctor(), gates(), logs() (+10 more)

### Community 20 - "commit.rs"
Cohesion: 0.21
Nodes (16): a_well_formed_message_passes(), branch_name(), branch_names_carry_the_goal_and_a_task_suffix(), missing_separator_is_rejected(), overlong_summaries_are_rejected(), scope_is_optional_and_may_contain_dashes(), slugify(), slugs_are_ascii_bounded_and_dash_separated() (+8 more)

### Community 22 - "parser.rs"
Cohesion: 0.13
Nodes (29): GateStatus, Finding, ansi_sequences_are_stripped_before_parsing(), CARGO_ERROR, cargo_parser_extracts_errors_and_locations(), collect_cargo(), collect_generic(), collect_node() (+21 more)

### Community 23 - "Command"
Cohesion: 0.10
Nodes (21): Command, Abort, Ask, Doctor, Gates, Goal, Logs, Ping (+13 more)

### Community 24 - "pathbuf"
Cohesion: 0.21
Nodes (4): render_command(), Result, CommitOutcome, Worktree

### Community 25 - "worktrees.rs"
Cohesion: 0.23
Nodes (11): a_dirty_worktree_is_reported(), a_dirty_worktree_needs_force_to_be_discarded(), a_repository_without_commits_is_rejected(), a_worktree_isolates_changes_until_merge(), commit_and_branch_state_are_readable(), creating_a_worktree_over_an_existing_path_is_refused(), fixture(), git() (+3 more)

### Community 26 - "JC Studio"
Cohesion: 0.15
Nodes (13): Arquitectura, Cómo termina un run, Desarrollo, Estado, Instalación, JC Studio, Licencia, Orquestación (+5 more)

### Community 27 - "jcs-cli/src/client.rs"
Cohesion: 0.07
Nodes (17): Args, init_tracing(), main(), restrict_dir(), shutdown_signal(), prepare_socket(), restrict_permissions(), serve() (+9 more)

### Community 28 - "Contribuir a JC Studio"
Cohesion: 0.22
Nodes (8): Checklist de PR, Contribuir a JC Studio, Convenciones de código, Flujo de trabajo, Idioma, Mensajes de commit (Conventional Commits), Seguridad, Tests

### Community 29 - "Contrato con OpenCode"
Cohesion: 0.22
Nodes (8): Al actualizar OpenCode, Ciclo de vida observado (modelo gratuito, ~5 s), Contrato con OpenCode, Descubrimiento del servicio, Endpoints usados, Formas de respuesta (inconsistentes, a propósito), Permisos, Stream de eventos: `GET /api/event`

### Community 30 - "0001 — Daemon `jcsd` + cliente fino por Unix socket"
Cohesion: 0.33
Nodes (6): 0001 — Daemon `jcsd` + cliente fino por Unix socket, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 31 - "adr/README.md"
Cohesion: 0.36
Nodes (3): ADRs — JC Studio, Cómo añadir una ADR, Regla de oro

### Community 32 - "0002 — OpenCode v2 HTTP API como backend de agentes (modo attach)"
Cohesion: 0.25
Nodes (7): 0002 — OpenCode v2 HTTP API como backend de agentes (modo attach), Alternativas consideradas, Consecuencias, Contexto, Decisión, Enmienda (2026-11-21): reconciliación y formularios, Verificación

### Community 33 - "0003 — SQLite + sqlx con event log append-only"
Cohesion: 0.22
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

### Community 41 - "plans.rs"
Cohesion: 0.18
Nodes (9): a_failed_dependency_blocks_its_dependents_forever(), a_plan_materializes_into_tasks_with_dependencies(), an_invalid_plan_is_refused_before_any_row_is_written(), counts_describe_whether_a_goal_settled(), fixture(), only_tasks_with_satisfied_dependencies_are_ready(), spec(), Store (+1 more)

### Community 45 - "loader.rs"
Cohesion: 0.15
Nodes (24): a_missing_instruction_file_is_an_error(), built_in_packs_load_and_validate(), BUILTIN, builtin_packs(), compute_hash(), disk_packs_override_built_ins_by_id(), Embedded, instruction_files_cannot_escape_the_pack_directory() (+16 more)

### Community 49 - "Result"
Cohesion: 0.15
Nodes (9): GoalStatus, StatusMachine, Result, GoalSettings, goal_columns!, GateResult, GateResultRow, run_columns! (+1 more)

### Community 50 - "jcs-proto/src/lib.rs"
Cohesion: 0.10
Nodes (21): AbortRequest, AgentHealth, ErrorBody, errors_carry_a_code_and_a_message(), EventDto, GateOutcomeView, GateRunRequest, GateRunResponse (+13 more)

### Community 51 - "path"
Cohesion: 0.14
Nodes (4): Evaluation, run_all(), GateOutcome, GateSet

### Community 52 - "chaos.sh"
Cohesion: 0.24
Nodes (10): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, chaos.sh script, start_daemon(), step() (+2 more)

### Community 53 - "e2e.sh"
Cohesion: 0.25
Nodes (9): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e.sh script, step(), wait_for_daemon() (+1 more)

### Community 54 - "mod.rs"
Cohesion: 0.14
Nodes (11): Action, Ask, RenderPolicy, ResumeGoal, event_loop(), execute(), FRAME, handle_key() (+3 more)

### Community 56 - "Store"
Cohesion: 0.14
Nodes (9): count_events(), events_for_task(), events_since(), latest_event_ts(), latest_seq(), recent_events(), row_to_event(), decode_ts() (+1 more)

### Community 57 - "jcs-policy/src/model.rs"
Cohesion: 0.17
Nodes (15): compose(), gate_sources_are_recorded(), instructions_follow_pack_order_and_keep_provenance(), later_packs_override_earlier_rules(), Overrides, project_overrides_win_over_every_pack(), rules_and_gates_can_be_disabled(), rust_policy() (+7 more)

### Community 58 - "PolicyError"
Cohesion: 0.12
Nodes (18): PolicyError, Cycle, InvalidGate, InvalidPack, Io, MissingPack, PathEscape, Refused (+10 more)

### Community 59 - "jcs-policy/src/render.rs"
Cohesion: 0.21
Nodes (20): sha256_hex(), agents_md_carries_the_marker_packs_and_rule_provenance(), changing_a_rule_changes_the_rendered_output(), composition(), content_of(), GateDocument, gates_toml_round_trips_to_the_same_gate_set(), GatesDocument (+12 more)

### Community 60 - "render_project.rs"
Cohesion: 0.32
Nodes (15): apply(), compile(), a_hand_written_agents_md_is_never_silently_replaced(), a_project_without_a_policy_file_uses_the_defaults(), an_unknown_pack_fails_with_an_actionable_message(), apply_creates_then_reports_unchanged(), check_passes_right_after_apply_and_fails_after_a_hand_edit(), check_reports_missing_files_before_the_first_apply() (+7 more)

### Community 61 - "Endpoint"
Cohesion: 0.17
Nodes (7): base_url_loses_trailing_slashes(), debug_never_prints_the_password(), Endpoint, service_password(), service_url(), opencode_config_dir(), Result

### Community 62 - "jcs-agent/src/client.rs"
Cohesion: 0.12
Nodes (14): AgentSession, api_errors_surface_the_server_message(), CreateSession, data_envelope_is_unwrapped_but_bare_payloads_survive(), extract_error_message(), lists_are_extracted_from_both_shapes(), model_reference_parses_and_roundtrips(), ModelRef (+6 more)

### Community 63 - "Project"
Cohesion: 0.07
Nodes (50): Goal, Project, cleanup(), ensure_integration_branch(), integration_worktree(), integration_worktree_path(), MERGE_LEASE_TTL, merge_task() (+42 more)

### Community 64 - "jcs-agent/src/events.rs"
Cohesion: 0.28
Nodes (10): decodes_the_real_captured_execution_event(), decodes_the_real_captured_usage_event(), extracts_text_deltas_only_from_delta_events(), frame(), non_json_payloads_are_an_error_with_context(), tool_events_are_read_defensively(), ToolInvocation, truncate() (+2 more)

### Community 65 - "jcs-policy/src/lib.rs"
Cohesion: 0.15
Nodes (13): ApplyReport, FileReport, FileState, Created, Drifted, Missing, Refused, Unchanged (+5 more)

### Community 66 - "runner.rs"
Cohesion: 0.26
Nodes (16): a_failing_command_fails_with_its_exit_code(), a_missing_program_is_an_error_not_a_pass(), a_successful_command_passes(), a_timeout_kills_the_child(), a_timeout_kills_the_whole_process_group(), both_streams_are_captured(), coverage_can_fail_a_command_that_exited_zero(), cwd() (+8 more)

### Community 67 - "StoreError"
Cohesion: 0.13
Nodes (10): StoreError, Conflict, Corrupt, Db, Domain, Io, Json, Locked (+2 more)

### Community 68 - "ui.rs"
Cohesion: 0.12
Nodes (4): an_empty_dashboard_renders_every_pane(), rendered(), the_input_modal_is_rendered_when_open(), the_selected_goal_and_its_tasks_are_visible()

### Community 69 - "e2e-policy.sh"
Cohesion: 0.18
Nodes (11): fail(), JCS_DB, JCS_DEFAULT_PACKS, JCS_MAX_CONCURRENT, JCS_MAX_REPAIR_CYCLES, JCS_PACKS_DIR, JCS_SOCKET, RUST_LOG (+3 more)

### Community 70 - "jcs-core/src/model.rs"
Cohesion: 0.15
Nodes (6): duration_requires_an_end(), only_process_failures_are_retryable(), Run, sample_run(), RunStatus, RunRow

### Community 71 - "AgentEvent"
Cohesion: 0.18
Nodes (6): EventStream, AgentEvent, ExecutionOutcome, Failed, Interrupted, Succeeded

### Community 72 - ".pool"
Cohesion: 0.26
Nodes (5): fixture(), integration_branch_is_stored_on_the_goal(), merge_failures_are_recorded_without_losing_the_task(), only_succeeded_unmerged_tasks_are_pending_merge(), Store

### Community 73 - "app.rs"
Cohesion: 0.19
Nodes (12): events_update_the_local_projection(), format_event(), goal(), goals_are_filtered_by_the_selected_project(), LOG_LINES, project(), selection_is_clamped_when_the_list_shrinks(), shift() (+4 more)

### Community 74 - "leases.rs"
Cohesion: 0.22
Nodes (7): a_lease_can_be_taken_only_once(), an_expired_lease_is_reclaimed(), expiring_leases_is_batched_and_audited(), fixture(), Lease, only_the_holder_can_renew_or_release(), Store

### Community 75 - "jcs-gates/src/model.rs"
Cohesion: 0.20
Nodes (12): coverage_threshold_only_makes_sense_with_the_coverage_parser(), empty_commands_are_rejected(), ensure_directory(), gate(), GateDef, GateError, Invalid, Io (+4 more)

### Community 76 - "Pendientes y riesgos aceptados"
Cohesion: 0.29
Nodes (5): Decisiones diferidas (no son deuda, son alcance), Limitaciones conocidas, Pendientes y riesgos aceptados, Riesgos aceptados, RUSTSEC-2026-0459 — SQLite empaquetado en `sqlx` 0.9

### Community 78 - "0007 — Packs en TOML, composición determinista y bucle de reparación"
Cohesion: 0.33
Nodes (6): 0007 — Packs en TOML, composición determinista y bucle de reparación, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 79 - "draw"
Cohesion: 0.30
Nodes (12): status_style(), centered_fixed(), draw(), draw_details(), draw_goals(), draw_input(), draw_log(), draw_projects() (+4 more)

### Community 81 - "ask"
Cohesion: 0.29
Nodes (7): ask(), fail_planning(), first_line(), spawn_planning(), titles_come_from_the_first_line_and_are_bounded(), AskRequest, AskResponse

### Community 82 - "executor.rs"
Cohesion: 0.07
Nodes (39): block_for_input(), build_prompt(), cancel_pending_forms(), commit_message(), commit_messages_are_always_valid(), decide(), describe_form(), fail_task() (+31 more)

### Community 89 - "GoalView"
Cohesion: 0.16
Nodes (13): goal_detail(), goal_view(), run_view(), run_views_expose_cost_and_tokens(), task_view(), GateView, GoalDetailResponse, GoalView (+5 more)

### Community 95 - "PlanError"
Cohesion: 0.15
Nodes (14): detect_cycle(), PlanError, AbsolutePath, Cycle, DuplicateKey, Empty, EmptyDescription, EmptyTitle (+6 more)

### Community 96 - "gates"
Cohesion: 0.23
Nodes (10): compile_policy(), file_state_label(), gate_views(), gates(), parser_label(), policy(), ProjectQuery, render_policy() (+2 more)

### Community 97 - "plan.rs"
Cohesion: 0.44
Nodes (11): a_valid_plan_passes(), cycles_are_rejected_with_the_chain(), duplicate_and_invalid_keys_are_rejected(), empty_and_oversized_plans_are_rejected(), empty_titles_and_descriptions_are_rejected(), path_claims_cannot_escape_or_claim_the_world(), plan(), plans_roundtrip_through_json() (+3 more)

### Community 101 - "e2e-merge.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-merge.sh script, step(), XDG_DATA_HOME

### Community 102 - "e2e-plan.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-plan.sh script, step(), XDG_DATA_HOME

### Community 103 - "ProjectView"
Cohesion: 0.32
Nodes (7): add_project(), project_view(), update_project(), AddProjectRequest, ProjectListResponse, ProjectResponse, ProjectView

### Community 105 - "CoreError"
Cohesion: 0.29
Nodes (5): CoreError, EmptyField, InvalidId, InvalidTransition, InvalidValue

### Community 106 - "TaskRole"
Cohesion: 0.29
Nodes (6): TaskRole, Docs, Implement, Migrate, Review, Test

### Community 109 - "ParserKind"
Cohesion: 0.33
Nodes (6): ParserKind, Cargo, Coverage, ExitCode, Generic, Node

### Community 110 - "0008 — Orquestador durable: plan, DAG, leases y rama de integración"
Cohesion: 0.33
Nodes (6): 0008 — Orquestador durable: plan, DAG, leases y rama de integración, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 112 - "ProjectCommand"
Cohesion: 0.40
Nodes (5): ProjectCommand, Add, List, Remove, Settings

### Community 113 - "Focus"
Cohesion: 0.40
Nodes (4): Focus, Goals, Projects, Tasks

### Community 114 - "Severity"
Cohesion: 0.50
Nodes (4): Severity, Error, Info, Warning

## Knowledge Gaps
- **138 isolated node(s):** `Project`, `Ask`, `Status`, `Logs`, `Runs` (+133 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 497 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **46 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppState` connect `AppState` to `gates`, `http.rs`, `ProjectView`, `state.rs`, `loader.rs`, `paths.rs`, `ask`, `executor.rs`, `path`, `Store`, `GoalView`, `jcs-cli/src/client.rs`, `Project`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **What connects `Project`, `Ask`, `Status` to the rest of the system?**
  _138 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `src/store.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.14705882352941177 - nodes in this community are weakly interconnected._
- **Why does `Store` connect `Store` to `src/store.rs`, `state.rs`, `.pool`, `leases.rs`, `plans.rs`, `paths.rs`, `AppState`, `tests/store.rs`, `Result`, `executor.rs`, `jcs-cli/src/client.rs`?**
  _High betweenness centrality (0.049) - this node is a cross-community bridge._
- **Should `Repo` be split into smaller, more focused modules?**
  _Cohesion score 0.12903225806451613 - nodes in this community are weakly interconnected._
- **Why does `EventKind` connect `event.rs` to `http.rs`, `render.rs`, `.pool`, `app.rs`, `leases.rs`, `plans.rs`, `tests/store.rs`, `Result`, `executor.rs`, `jcs-cli/src/main.rs`, `Store`, `jcs-cli/src/client.rs`, `Project`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **Should `commit.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.12380952380952381 - nodes in this community are weakly interconnected._