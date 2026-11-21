# Graph Report - jc-studio  (2026-11-21)

## Corpus Check
- 161 files · ~117,303 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 13 file(s) not represented in the graph (top: .toml 8, (none) 3, .desktop 1)

## Summary
- 2508 nodes · 5542 edges · 202 communities (118 shown, 84 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 207 edges (avg confidence: 0.84)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `a369fed6`
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
- .pool
- jobs
- event.rs
- sse.rs
- paths.rs
- ApiResult
- languages.rs
- jcs-cli/src/main.rs
- commit.rs
- parser.rs
- Command
- repo.rs
- worktrees.rs
- JC Studio
- jcs-client/src/lib.rs
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
- StatusMachine
- jcs-proto/src/lib.rs
- GateStatus
- chaos.sh
- e2e.sh
- App
- check_contract.py
- Store
- jcs-policy/src/model.rs
- jcs-policy/src/lib.rs
- jcs-policy/src/render.rs
- render_project.rs
- Endpoint
- jcs-agent/src/client.rs
- planner.rs
- jcs-agent/src/events.rs
- jcs-policy/src/lib.rs
- Project
- search.rs
- code.rs
- e2e-policy.sh
- encode
- src/app.rs
- .pool
- app.rs
- leases.rs
- indexer.rs
- MemoryError
- context.rs
- 0007 — Packs en TOML, composición determinista y bucle de reparación
- Language
- dag.rs
- SymbolKind
- executor.rs
- core-git/instructions.md
- core-security/instructions.md
- postgres-schema/instructions.md
- rust-service/instructions.md
- testing-tdd/instructions.md
- ts-frontend/instructions.md
- Store
- reviewer.rs
- chunk.rs
- Update
- path
- Json
- terminal.rs
- e2e-merge.sh
- e2e-plan.sh
- ProjectView
- jcs-daemon/src/settings.rs
- telegram.rs
- e2e-memory.sh
- secrets.rs
- EventKind
- jcs-app/src/state.rs
- 0008 — Orquestador durable: plan, DAG, leases y rama de integración
- jcs-client/src/lib.rs
- 0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado
- ParserKind
- steps
- Command
- tui/mod.rs
- runner.rs
- jcs-gates/src/model.rs
- HANDOFF — leer esto primero
- jcs-memory/src/boundaries.rs
- StoreError
- tests/store.rs
- soak.sh
- PolicyError
- STATUS.md
- Estado del proyecto
- Studio
- AppState
- State
- Runbook
- 0010 — Cliente de escritorio egui con terminales embebidas
- style
- ui/mod.rs
- RuntimeSettings
- goal_detail
- install.sh
- licenses/README.md
- package.sh
- super
- deserialize
- tests/memory.rs
- daemon.rs
- ui/settings.rs
- status.rs
- router
- trace.rs
- e2e-multigoal.sh
- jcs-daemon/src/main.rs
- 0011 — Varios objetivos a la vez, con reparto justo y claims de rutas
- 0012 — Secretos en archivo 0600, fuera de SQLite y de la API
- jcs-core/src/boundaries.rs
- config.rs

## God Nodes (most connected - your core abstractions)
1. `AppState` - 118 edges
2. `Studio` - 68 edges
3. `Client` - 59 edges
4. `Store` - 56 edges
5. `Project` - 47 edges
6. `Repo` - 43 edges
7. `State` - 42 edges
8. `Update` - 41 edges
9. `App` - 38 edges
10. `Goal` - 37 edges

## Surprising Connections (you probably didn't know these)
- `Studio` --references--> `Terminals`  [INFERRED]
  crates/jcs-app/src/app.rs → crates/jcs-app/src/terminal.rs
- `Studio` --references--> `CodeState`  [INFERRED]
  crates/jcs-app/src/app.rs → crates/jcs-app/src/ui/code.rs
- `panel()` --calls--> `task_progress()`  [INFERRED]
  crates/jcs-app/src/ui/trace.rs → crates/jcs-app/src/state.rs
- `graph()` --calls--> `truncate()`  [INFERRED]
  crates/jcs-app/src/ui/dag.rs → crates/jcs-app/src/ui/mod.rs
- `goal()` --calls--> `metric()`  [INFERRED]
  crates/jcs-app/src/ui/details.rs → crates/jcs-app/src/ui/mod.rs

## Import Cycles
- 1-file cycle: `crates/jcs-app/src/daemon.rs -> crates/jcs-app/src/daemon.rs`

## Communities (202 total, 84 thin omitted)

### Community 0 - "src/store.rs"
Cohesion: 0.13
Nodes (4): append_event(), concurrent_appends_do_not_lock_the_database(), encode_timestamp(), is_unique_violation()

### Community 1 - "OpenCodeClient"
Cohesion: 0.18
Nodes (5): AgentSession, CreateSession, into_list(), OpenCodeClient, unwrap_data()

### Community 3 - "Task"
Cohesion: 0.23
Nodes (4): Task, encode_ts(), insert_event(), Store

### Community 4 - "commit.rs"
Cohesion: 0.12
Nodes (13): builder_rejects_a_bad_summary(), builder_renders_header_and_body(), CommitKind, Chore, Ci, Docs, Feat, Fix (+5 more)

### Community 6 - "http.rs"
Cohesion: 0.09
Nodes (18): a_planning_failure_blocks_the_goal_instead_of_leaving_it_pending(), ask(), events(), EventsQuery, fail_planning(), first_line(), LIST_LIMIT, metrics() (+10 more)

### Community 7 - "render.rs"
Cohesion: 0.22
Nodes (10): duration(), durations_are_human_scaled(), goal_detail(), plural(), plurals_read_naturally(), run_detail(), short_id(), short_ids_drop_the_prefix_and_bound_the_length() (+2 more)

### Community 8 - "ApiError"
Cohesion: 0.10
Nodes (16): AgentError, Api, Decode, Discovery, Io, Json, Stream, Transport (+8 more)

### Community 9 - ".pool"
Cohesion: 0.10
Nodes (7): GoalStatus, GoalRow, GoalSettings, goal_columns!, Store, Store, Store

### Community 11 - "jobs"
Cohesion: 0.08
Nodes (30): name, runs-on, steps, name, runs-on, steps, env, CARGO_TERM_COLOR (+22 more)

### Community 12 - "event.rs"
Cohesion: 0.13
Nodes (4): builder_sets_scopes_and_payload(), event_kinds!, NewEvent, timestamps_encode_with_fixed_precision_utc()

### Community 13 - "sse.rs"
Cohesion: 0.21
Nodes (16): a_trailing_lone_cr_waits_for_the_next_chunk(), accepts_crlf_and_bare_cr_line_endings(), blank_lines_between_comment_only_blocks_do_not_dispatch(), captures_event_and_id_fields_when_present(), data_without_space_after_colon_is_valid(), decode_all(), decodes_a_minimal_data_only_event(), find_line_end() (+8 more)

### Community 14 - "paths.rs"
Cohesion: 0.33
Nodes (10): SOCKET_NAME, data_dir(), database_lives_under_the_data_directory(), database_path(), log_dir(), runtime_dir(), socket_lives_under_the_runtime_directory(), socket_path() (+2 more)

### Community 15 - "ApiResult"
Cohesion: 0.15
Nodes (19): ApiResult, compile_policy(), event_history(), file_state_label(), gate_views(), gates(), HistoryQuery, memory_search() (+11 more)

### Community 16 - "languages.rs"
Cohesion: 0.19
Nodes (30): callee_name(), callee_names_are_reduced_to_the_last_segment(), collect_imports(), extract(), extract_python(), extract_rust(), extract_rust_fixture(), extract_sql() (+22 more)

### Community 19 - "jcs-cli/src/main.rs"
Cohesion: 0.17
Nodes (22): ask(), AskArgs, Cli, display_command(), doctor(), follow_goal(), gates(), index() (+14 more)

### Community 20 - "commit.rs"
Cohesion: 0.21
Nodes (16): a_well_formed_message_passes(), branch_name(), branch_names_carry_the_goal_and_a_task_suffix(), missing_separator_is_rejected(), overlong_summaries_are_rejected(), scope_is_optional_and_may_contain_dashes(), slugify(), slugs_are_ascii_bounded_and_dash_separated() (+8 more)

### Community 22 - "parser.rs"
Cohesion: 0.12
Nodes (32): Finding, Severity, Error, Info, Warning, ansi_sequences_are_stripped_before_parsing(), CARGO_ERROR, cargo_parser_extracts_errors_and_locations() (+24 more)

### Community 23 - "Command"
Cohesion: 0.07
Nodes (30): Command, Abort, Ask, Doctor, Gates, Goal, Index, Logs (+22 more)

### Community 24 - "repo.rs"
Cohesion: 0.31
Nodes (4): render_command(), Result, CommitOutcome, Worktree

### Community 25 - "worktrees.rs"
Cohesion: 0.23
Nodes (11): a_dirty_worktree_is_reported(), a_dirty_worktree_needs_force_to_be_discarded(), a_repository_without_commits_is_rejected(), a_worktree_isolates_changes_until_merge(), commit_and_branch_state_are_readable(), creating_a_worktree_over_an_existing_path_is_refused(), fixture(), git() (+3 more)

### Community 26 - "JC Studio"
Cohesion: 0.12
Nodes (16): App de escritorio, Arquitectura, Cómo termina un run, Desarrollo, Estado, Instalación, JC Studio, Licencia (+8 more)

### Community 27 - "jcs-client/src/lib.rs"
Cohesion: 0.21
Nodes (4): prepare_socket(), restrict_permissions(), serve(), serve_connection()

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
Cohesion: 0.33
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
Cohesion: 0.35
Nodes (12): jcs-agent, jcs-app, jcs-cli, jcs-client, jcs-core, jcs-daemon, jcs-gates, jcs-git (+4 more)

### Community 41 - "plans.rs"
Cohesion: 0.07
Nodes (42): a_valid_plan_passes(), cycles_are_rejected_with_the_chain(), detect_cycle(), duplicate_and_invalid_keys_are_rejected(), empty_and_oversized_plans_are_rejected(), empty_titles_and_descriptions_are_rejected(), path_claims_cannot_escape_or_claim_the_world(), Plan (+34 more)

### Community 45 - "loader.rs"
Cohesion: 0.17
Nodes (24): a_missing_instruction_file_is_an_error(), built_in_packs_load_and_validate(), BUILTIN, builtin_packs(), compute_hash(), disk_packs_override_built_ins_by_id(), Embedded, instruction_files_cannot_escape_the_pack_directory() (+16 more)

### Community 49 - "StatusMachine"
Cohesion: 0.09
Nodes (12): EVENT_SCHEMA_VERSION, MAX_PLAN_TASKS, failed_task_can_only_retry_or_cancel(), GateStatus, RunStatus, StatusMachine, TaskStatus, transition_error_names_the_entity_and_both_states() (+4 more)

### Community 50 - "jcs-proto/src/lib.rs"
Cohesion: 0.10
Nodes (25): AbortRequest, AgentHealth, ErrorBody, errors_carry_a_code_and_a_message(), EventDto, GateOutcomeView, GateRunResponse, GateView (+17 more)

### Community 51 - "GateStatus"
Cohesion: 0.21
Nodes (3): evaluate(), Evaluation, ScanOptions

### Community 52 - "chaos.sh"
Cohesion: 0.24
Nodes (10): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, chaos.sh script, start_daemon(), step() (+2 more)

### Community 53 - "e2e.sh"
Cohesion: 0.25
Nodes (9): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e.sh script, step(), wait_for_daemon() (+1 more)

### Community 54 - "App"
Cohesion: 0.07
Nodes (22): App, Focus, Goals, Projects, Tasks, shift(), status_style(), an_empty_dashboard_renders_every_pane() (+14 more)

### Community 56 - "Store"
Cohesion: 0.24
Nodes (9): Event, count_events(), events_for_task(), events_since(), latest_event_ts(), latest_seq(), recent_events(), row_to_event() (+1 more)

### Community 57 - "jcs-policy/src/model.rs"
Cohesion: 0.15
Nodes (16): compose(), gate_sources_are_recorded(), instructions_follow_pack_order_and_keep_provenance(), later_packs_override_earlier_rules(), Overrides, project_overrides_win_over_every_pack(), rules_and_gates_can_be_disabled(), rust_policy() (+8 more)

### Community 58 - "jcs-policy/src/lib.rs"
Cohesion: 0.23
Nodes (10): a_policy_round_trips(), an_invalid_boundary_is_rejected_with_a_reason(), boundary_rules_are_parsed_and_validated(), POLICY_FILE, ProjectPolicy, read(), secret_scanning_is_enabled_by_default_and_can_be_scoped(), SecretsPolicy (+2 more)

### Community 59 - "jcs-policy/src/render.rs"
Cohesion: 0.20
Nodes (20): Compiled, agents_md_carries_the_marker_packs_and_rule_provenance(), changing_a_rule_changes_the_rendered_output(), composition(), content_of(), GateDocument, gates_toml_round_trips_to_the_same_gate_set(), GatesDocument (+12 more)

### Community 60 - "render_project.rs"
Cohesion: 0.32
Nodes (15): apply(), compile(), a_hand_written_agents_md_is_never_silently_replaced(), a_project_without_a_policy_file_uses_the_defaults(), an_unknown_pack_fails_with_an_actionable_message(), apply_creates_then_reports_unchanged(), check_passes_right_after_apply_and_fails_after_a_hand_edit(), check_reports_missing_files_before_the_first_apply() (+7 more)

### Community 61 - "Endpoint"
Cohesion: 0.17
Nodes (7): base_url_loses_trailing_slashes(), debug_never_prints_the_password(), Endpoint, service_password(), service_url(), opencode_config_dir(), Result

### Community 62 - "jcs-agent/src/client.rs"
Cohesion: 0.10
Nodes (14): api_errors_surface_the_server_message(), data_envelope_is_unwrapped_but_bare_payloads_survive(), EventStream, extract_error_message(), lists_are_extracted_from_both_shapes(), model_reference_parses_and_roundtrips(), ModelInfo, ModelRef (+6 more)

### Community 63 - "planner.rs"
Cohesion: 0.17
Nodes (18): assistant_text(), assistant_text_reads_the_newest_message_and_ignores_reasoning(), braces_inside_strings_do_not_close_the_object(), cancel_forms(), collect_response(), escaped_quotes_do_not_confuse_the_scanner(), extract_json_object(), extracts_bare_json() (+10 more)

### Community 64 - "jcs-agent/src/events.rs"
Cohesion: 0.14
Nodes (17): AgentEvent, decodes_the_real_captured_execution_event(), decodes_the_real_captured_usage_event(), ExecutionOutcome, Failed, Interrupted, Succeeded, extracts_reasoning_deltas_separately_from_text() (+9 more)

### Community 65 - "jcs-policy/src/lib.rs"
Cohesion: 0.15
Nodes (13): ApplyReport, FileReport, FileState, Created, Drifted, Missing, Refused, Unchanged (+5 more)

### Community 66 - "Project"
Cohesion: 0.17
Nodes (19): duration_requires_an_end(), Goal, only_process_failures_are_retryable(), Project, sample_run(), policy_sections(), with_memory_context(), cleanup() (+11 more)

### Community 67 - "search.rs"
Cohesion: 0.12
Nodes (27): cosine(), cosine_identifies_identical_and_orthogonal_vectors(), decode_vector(), EmbeddingFormat, Ollama, OpenAi, mismatched_dimensions_are_not_compared(), parse_vector() (+19 more)

### Community 68 - "code.rs"
Cohesion: 0.12
Nodes (23): identifiers_are_shortened_for_display(), short(), a_file_is_read_and_saved(), build_tree(), CodeState, desired_root(), diff_panel(), editor() (+15 more)

### Community 69 - "e2e-policy.sh"
Cohesion: 0.18
Nodes (11): fail(), JCS_DB, JCS_DEFAULT_PACKS, JCS_MAX_CONCURRENT, JCS_MAX_REPAIR_CYCLES, JCS_PACKS_DIR, JCS_SOCKET, RUST_LOG (+3 more)

### Community 71 - "src/app.rs"
Cohesion: 0.08
Nodes (19): AddProjectForm, ConsoleTab, Agent, Events, Terminals, GlobalForm, number(), parse_chat_ids() (+11 more)

### Community 72 - ".pool"
Cohesion: 0.23
Nodes (5): fixture(), integration_branch_is_stored_on_the_goal(), merge_failures_are_recorded_without_losing_the_task(), only_succeeded_unmerged_tasks_are_pending_merge(), Store

### Community 73 - "app.rs"
Cohesion: 0.21
Nodes (11): events_update_the_local_projection(), format_event(), goal(), goals_are_filtered_by_the_selected_project(), LOG_LINES, project(), selection_is_clamped_when_the_list_shrinks(), short() (+3 more)

### Community 74 - "leases.rs"
Cohesion: 0.22
Nodes (7): a_lease_can_be_taken_only_once(), an_expired_lease_is_reclaimed(), expiring_leases_is_batched_and_audited(), fixture(), Lease, only_the_holder_can_renew_or_release(), Store

### Community 75 - "indexer.rs"
Cohesion: 0.14
Nodes (18): Embeddings, encode_vector(), clear_project(), embed_chunks(), file_order_is_deterministic(), index_file(), index_project(), irrelevant_and_hidden_directories_are_skipped() (+10 more)

### Community 76 - "MemoryError"
Cohesion: 0.22
Nodes (8): MemoryError, Db, Embedding, Invalid, Io, Json, Parse, Result

### Community 77 - "context.rs"
Cohesion: 0.22
Nodes (9): a_hit_renders_with_a_citation_and_the_right_fence(), build(), ContextOptions, ContextPackage, hit(), language_fence(), render_hit(), unknown_extensions_get_a_plain_fence() (+1 more)

### Community 78 - "0007 — Packs en TOML, composición determinista y bucle de reparación"
Cohesion: 0.33
Nodes (6): 0007 — Packs en TOML, composición determinista y bucle de reparación, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 79 - "Language"
Cohesion: 0.13
Nodes (11): HitSource, Fallback, Graph, Lexical, Semantic, Language, JavaScript, Python (+3 more)

### Community 80 - "dag.rs"
Cohesion: 0.12
Nodes (18): task_progress(), TaskProgress, card(), composer(), goals(), GAP_X, GAP_Y, graph() (+10 more)

### Community 81 - "SymbolKind"
Cohesion: 0.13
Nodes (13): SymbolKind, Class, Constant, Enum, Function, Index, Interface, Module (+5 more)

### Community 82 - "executor.rs"
Cohesion: 0.05
Nodes (57): block_for_input(), build_prompt(), cancel_pending_forms(), commit_message(), commit_messages_are_always_valid(), decide(), describe_form(), fail_task() (+49 more)

### Community 90 - "Store"
Cohesion: 0.13
Nodes (3): Run, RunRow, Store

### Community 92 - "reviewer.rs"
Cohesion: 0.13
Nodes (23): a_huge_diff_is_truncated_with_a_note(), a_valid_verdict_is_parsed(), a_warning_only_verdict_does_not_block(), findings_are_capped(), findings_must_cite_a_real_location(), MAX_ATTEMPTS, MAX_DIFF_CHARS, parse_verdict() (+15 more)

### Community 95 - "chunk.rs"
Cohesion: 0.25
Nodes (15): a_file_without_symbols_still_yields_a_header_chunk(), a_header_comment_before_the_first_symbol_is_chunked(), a_symbol_starting_on_line_one_gets_no_empty_header(), chunk_file(), chunk_hashes_change_with_content_and_position(), chunking_is_deterministic(), chunks_are_aligned_to_symbols_and_contained_symbols_are_skipped(), oversized_symbols_are_split_with_overlap() (+7 more)

### Community 96 - "Update"
Cohesion: 0.14
Nodes (24): connection_loop(), handle(), notice(), refresh(), Update, Event, Gates, GoalAction (+16 more)

### Community 97 - "path"
Cohesion: 0.29
Nodes (5): EdgeKind, Call, Import, References, Tests

### Community 99 - "Json"
Cohesion: 0.14
Nodes (18): abort(), catalog_models(), health(), list_goals(), list_projects(), list_runs(), list_tasks(), memory_index() (+10 more)

### Community 100 - "terminal.rs"
Cohesion: 0.11
Nodes (7): a_command_runs_and_reaches_the_screen(), color(), nonzero(), render(), Session, Terminals, the_256_colour_cube_maps_to_rgb()

### Community 101 - "e2e-merge.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-merge.sh script, step(), XDG_DATA_HOME

### Community 102 - "e2e-plan.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-plan.sh script, step(), XDG_DATA_HOME

### Community 103 - "ProjectView"
Cohesion: 0.40
Nodes (5): add_project(), project_view(), update_project(), AddProjectRequest, ProjectResponse

### Community 104 - "jcs-daemon/src/settings.rs"
Cohesion: 0.22
Nodes (14): Config, a_patch_only_touches_what_it_names(), an_empty_string_clears_a_model(), concurrency_needs_a_restart_but_is_saved(), config(), environment_is_the_initial_value(), invalid_ranges_are_rejected_and_leave_the_value_untouched(), set_number() (+6 more)

### Community 105 - "telegram.rs"
Cohesion: 0.13
Nodes (25): ask_requires_both_parts(), ask_text(), Command, Ask, Goal, Help, Projects, Resume (+17 more)

### Community 106 - "e2e-memory.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-memory.sh script, step(), XDG_DATA_HOME

### Community 107 - "secrets.rs"
Cohesion: 0.14
Nodes (24): allow_prefixes_suppress_whole_directories(), BENIGN_MARKERS, binary_and_oversized_files_are_skipped(), candidates(), entropy(), ENTROPY_MIN_LEN, ENTROPY_THRESHOLD, entropy_threshold_separates_random_from_words() (+16 more)

### Community 108 - "EventKind"
Cohesion: 0.10
Nodes (9): CoreError, EmptyField, InvalidId, InvalidTransition, InvalidValue, EventKind, Result, task_columns! (+1 more)

### Community 109 - "jcs-app/src/state.rs"
Cohesion: 0.17
Nodes (12): Connection, Connecting, Offline, Online, errors_show_reason_and_hint(), event(), events_keep_the_goal_they_belong_to(), replayed_history_does_not_duplicate_lines() (+4 more)

### Community 110 - "0008 — Orquestador durable: plan, DAG, leases y rama de integración"
Cohesion: 0.33
Nodes (6): 0008 — Orquestador durable: plan, DAG, leases y rama de integración, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 112 - "jcs-client/src/lib.rs"
Cohesion: 0.15
Nodes (8): a_second_daemon_refuses_to_steal_a_live_socket(), a_stale_socket_from_a_killed_daemon_is_replaced(), Daemon, get_json(), spawn_daemon(), the_daemon_serves_health_and_history_over_a_socket(), wait_for_socket(), a_second_store_cannot_open_the_same_database()

### Community 113 - "0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado"
Cohesion: 0.33
Nodes (6): 0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 114 - "ParserKind"
Cohesion: 0.33
Nodes (6): ParserKind, Cargo, Coverage, ExitCode, Generic, Node

### Community 118 - "steps"
Cohesion: 0.09
Nodes (22): jobs, linux, name, runs-on, steps, name, on, push (+14 more)

### Community 124 - "Command"
Cohesion: 0.11
Nodes (18): Command, Abort, AddProject, Ask, IndexMemory, LoadSettings, MemoryStats, Policy (+10 more)

### Community 127 - "tui/mod.rs"
Cohesion: 0.13
Nodes (11): Action, Ask, RenderPolicy, ResumeGoal, event_loop(), execute(), FRAME, handle_key() (+3 more)

### Community 128 - "runner.rs"
Cohesion: 0.26
Nodes (16): a_failing_command_fails_with_its_exit_code(), a_missing_program_is_an_error_not_a_pass(), a_successful_command_passes(), a_timeout_kills_the_child(), a_timeout_kills_the_whole_process_group(), both_streams_are_captured(), coverage_can_fail_a_command_that_exited_zero(), cwd() (+8 more)

### Community 129 - "jcs-gates/src/model.rs"
Cohesion: 0.14
Nodes (15): run_all(), coverage_threshold_only_makes_sense_with_the_coverage_parser(), empty_commands_are_rejected(), ensure_directory(), gate(), GateDef, GateError, Invalid (+7 more)

### Community 130 - "HANDOFF — leer esto primero"
Cohesion: 0.11
Nodes (18): 5.1 Revisor con rúbrica — HECHO, 5.2 Escaneo de secretos — HECHO, 5.3 Fitness functions arquitectónicas — HECHO, 5.3 Revisor con rúbrica (modelo), 5.4 Enrutado por riesgo — HECHO, 5.4b Enrutado por riesgo (histórico), Cómo cerrar una sesión de trabajo, Entorno (+10 more)

### Community 131 - "jcs-memory/src/boundaries.rs"
Cohesion: 0.31
Nodes (15): BoundaryRule, a_clean_dependency_direction_passes(), a_forbidden_import_is_reported_with_its_line(), check(), describe(), files_under(), invalid_rules_are_ignored_instead_of_panicking(), mentions_module() (+7 more)

### Community 132 - "StoreError"
Cohesion: 0.13
Nodes (10): StoreError, Conflict, Corrupt, Db, Domain, Io, Json, Locked (+2 more)

### Community 133 - "tests/store.rs"
Cohesion: 0.25
Nodes (13): an_empty_gate_set_is_not_a_pass(), backup_produces_a_readable_snapshot(), crashed_runs_become_interrupted_and_retryable(), deleting_a_project_cascades_state_but_preserves_the_audit_trail(), event_log_is_monotonic_and_resumable(), fixture(), goal_and_task_transitions_are_enforced_by_the_store(), orphaned_tasks_are_parked_as_blocked_not_failed() (+5 more)

### Community 134 - "soak.sh"
Cohesion: 0.19
Nodes (12): fail(), goal_status(), JCS_DB, JCS_DEFAULT_PACKS, JCS_MAX_CONCURRENT, JCS_MEMORY_SWEEP_SECS, JCS_SOCKET, RUST_LOG (+4 more)

### Community 136 - "PolicyError"
Cohesion: 0.17
Nodes (11): PolicyError, Cycle, InvalidGate, InvalidPack, Io, MissingPack, PathEscape, Refused (+3 more)

### Community 137 - "STATUS.md"
Cohesion: 0.27
Nodes (5): Decisiones diferidas (no son deuda, son alcance), Limitaciones conocidas, Pendientes y riesgos aceptados, Riesgos aceptados, RUSTSEC-2026-0459 — SQLite empaquetado en `sqlx` 0.9

### Community 138 - "Estado del proyecto"
Cohesion: 0.18
Nodes (11): Avance, Estado del proyecto, Evidencia reproducible, Fase 3: qué falta exactamente, Fase 4: qué falta exactamente, Fase 5: qué falta exactamente, Fase 6: qué falta exactamente, Fase 7: qué falta exactamente (+3 more)

### Community 139 - "Studio"
Cohesion: 0.15
Nodes (4): Studio, panel(), metric(), panel()

### Community 140 - "AppState"
Cohesion: 0.10
Nodes (6): index_project(), IndexGuard, spawn_index(), spawn_sweeper(), AgentHandle, AppState

### Community 142 - "Runbook"
Cohesion: 0.17
Nodes (12): Comprobación, Configuración, Instalación, La app de escritorio, Nada de esto toca tu rama sin permiso, Operación diaria, Problemas frecuentes, Recuperación (+4 more)

### Community 145 - "0010 — Cliente de escritorio egui con terminales embebidas"
Cohesion: 0.33
Nodes (6): 0010 — Cliente de escritorio egui con terminales embebidas, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 148 - "ui/mod.rs"
Cohesion: 0.22
Nodes (14): Notice, central(), connection_badge(), console(), details(), events(), model_selector(), notice_bar() (+6 more)

### Community 150 - "goal_detail"
Cohesion: 0.40
Nodes (4): goal_detail(), goal_view(), task_view(), task_views()

### Community 176 - "super"
Cohesion: 0.15
Nodes (5): claims_overlap(), empty_claims_conflict_with_anything_running(), nested_prefixes_overlap_but_siblings_do_not(), paths_overlap(), id_type!

### Community 178 - "deserialize"
Cohesion: 0.21
Nodes (6): clearing_the_token_does_not_fall_back_to_the_environment(), Secrets, the_hint_never_contains_the_whole_token(), the_token_is_written_private_and_reloaded(), Values, write_private()

### Community 179 - "tests/memory.rs"
Cohesion: 0.24
Nodes (13): a_second_index_run_skips_everything_unchanged(), an_empty_or_missing_index_returns_no_hits_instead_of_failing(), context_packages_are_cited_and_budgeted(), deleted_files_disappear_from_the_index(), extra_sections_are_included_when_the_budget_allows(), fixture(), indexing_extracts_symbols_chunks_and_sql_tables(), only_changed_files_are_reindexed() (+5 more)

### Community 180 - "daemon.rs"
Cohesion: 0.15
Nodes (7): Daemon, MemoryHitView, MemorySearchResponse, MemoryStatsView, ProjectSettings, RenderedFileView, RenderPolicyResponse

### Community 181 - "ui/settings.rs"
Cohesion: 0.29
Nodes (10): budgets_must_be_positive_numbers(), empty_budget_means_no_ceiling(), parse_amount(), parse_tokens(), add_project_window(), daemon_settings(), project_settings(), restart_daemon() (+2 more)

### Community 182 - "status.rs"
Cohesion: 0.21
Nodes (4): any_goal(), any_run(), any_task(), status_enum!

### Community 183 - "router"
Cohesion: 0.36
Nodes (11): a_directory_that_is_not_a_repository_is_rejected(), a_registered_repository_appears_in_the_list(), a_relative_project_path_is_rejected_with_a_reason(), asking_for_an_unknown_project_suggests_the_next_step(), health_reports_the_protocol_version(), record_lifecycle(), router(), send() (+3 more)

### Community 184 - "trace.rs"
Cohesion: 0.33
Nodes (7): ActivityLine, truncate(), ActivityLine, block(), duration(), panel(), trace_line()

### Community 185 - "e2e-multigoal.sh"
Cohesion: 0.22
Nodes (9): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_MODEL, JCS_SOCKET, RUST_LOG, e2e-multigoal.sh script, step() (+1 more)

### Community 186 - "jcs-daemon/src/main.rs"
Cohesion: 0.36
Nodes (5): Args, init_tracing(), main(), restrict_dir(), shutdown_signal()

### Community 187 - "0011 — Varios objetivos a la vez, con reparto justo y claims de rutas"
Cohesion: 0.29
Nodes (6): 0011 — Varios objetivos a la vez, con reparto justo y claims de rutas, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 188 - "0012 — Secretos en archivo 0600, fuera de SQLite y de la API"
Cohesion: 0.29
Nodes (6): 0012 — Secretos en archivo 0600, fuera de SQLite y de la API, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 189 - "jcs-core/src/boundaries.rs"
Cohesion: 0.53
Nodes (4): absolute_and_escaping_paths_are_rejected(), empty_and_self_referential_rules_are_rejected(), relative_prefixes_are_accepted(), rule()

## Knowledge Gaps
- **230 isolated node(s):** `Succeeded`, `Failed`, `Interrupted`, `Events`, `Agent` (+225 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 807 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **84 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Client` connect `jcs-cli/src/main.rs` to `Update`, `.ask`, `.render_policy`, `.run_gates`, `Client`, `encode`, `jcs-client/src/lib.rs`, `.add_project`, `daemon.rs`, `.update_settings`, `tui/mod.rs`?**
  _High betweenness centrality (0.050) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `Studio` (e.g. with `Terminals` and `CodeState`) actually correct?**
  _`Studio` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Succeeded`, `Failed`, `Interrupted` to the rest of the system?**
  _230 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `src/store.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1323529411764706 - nodes in this community are weakly interconnected._
- **Why does `AppState` connect `AppState` to `http.rs`, `ApiResult`, `RuntimeSettings`, `goal_detail`, `jcs-client/src/lib.rs`, `loader.rs`, `deserialize`, `GateStatus`, `router`, `jcs-daemon/src/main.rs`, `planner.rs`, `jcs-agent/src/events.rs`, `Project`, `indexer.rs`, `executor.rs`, `reviewer.rs`, `Json`, `ProjectView`, `jcs-daemon/src/settings.rs`, `telegram.rs`?**
  _High betweenness centrality (0.042) - this node is a cross-community bridge._
- **Should `Repo` be split into smaller, more focused modules?**
  _Cohesion score 0.11764705882352941 - nodes in this community are weakly interconnected._
- **Why does `Store` connect `indexer.rs` to `src/store.rs`, `search.rs`, `tests/store.rs`, `.pool`, `.pool`, `leases.rs`, `plans.rs`, `AppState`, `context.rs`, `EventKind`, `StatusMachine`, `deserialize`, `executor.rs`, `tests/memory.rs`, `Store`, `jcs-daemon/src/main.rs`?**
  _High betweenness centrality (0.040) - this node is a cross-community bridge._