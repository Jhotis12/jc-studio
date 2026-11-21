# Graph Report - jc-studio  (2026-11-21)

## Corpus Check
- 153 files · ~108,070 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 13 file(s) not represented in the graph (top: .toml 8, (none) 3, .desktop 1)

## Summary
- 2374 nodes · 5209 edges · 180 communities (104 shown, 76 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 189 edges (avg confidence: 0.84)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `8741d4a2`
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
- resolve_project
- Json
- languages.rs
- jcs-cli/src/main.rs
- commit.rs
- parser.rs
- Command
- jcs-git/src/lib.rs
- worktrees.rs
- JC Studio
- path
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
- scheduler.rs
- e2e-policy.sh
- jcs-core/src/lib.rs
- Studio
- .pool
- app.rs
- leases.rs
- indexer.rs
- Embeddings
- context.rs
- 0007 — Packs en TOML, composición determinista y bucle de reparación
- Language
- jcs-client/src/lib.rs
- SymbolKind
- executor.rs
- core-git/instructions.md
- core-security/instructions.md
- postgres-schema/instructions.md
- rust-service/instructions.md
- testing-tdd/instructions.md
- ts-frontend/instructions.md
- goal_detail
- Store
- reviewer.rs
- chunk.rs
- Update
- path
- ids.rs
- Goal
- terminal.rs
- e2e-merge.sh
- e2e-plan.sh
- ProjectView
- RuntimeSettings
- telegram.rs
- e2e-memory.sh
- secrets.rs
- Command
- State
- 0008 — Orquestador durable: plan, DAG, leases y rama de integración
- plans.rs
- 0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado
- ParserKind
- steps
- ProjectCommand
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
- memory_service.rs
- EventKind
- Runbook
- 0010 — Cliente de escritorio egui con terminales embebidas
- .post
- style
- install.sh
- licenses/README.md
- package.sh

## God Nodes (most connected - your core abstractions)
1. `AppState` - 112 edges
2. `Client` - 58 edges
3. `Store` - 56 edges
4. `Studio` - 51 edges
5. `Project` - 48 edges
6. `Repo` - 44 edges
7. `Update` - 40 edges
8. `App` - 38 edges
9. `State` - 37 edges
10. `Goal` - 37 edges

## Surprising Connections (you probably didn't know these)
- `Studio` --references--> `Terminals`  [INFERRED]
  crates/jcs-app/src/app.rs → crates/jcs-app/src/terminal.rs
- `AppState` --references--> `IndexGuard`  [INFERRED]
  crates/jcs-daemon/src/state.rs → crates/jcs-daemon/src/memory_service.rs
- `packs()` --references--> `Pack`  [INFERRED]
  crates/jcs-policy/tests/render_project.rs → crates/jcs-policy/src/model.rs
- `compile()` --calls--> `compose()`  [INFERRED]
  crates/jcs-policy/src/lib.rs → crates/jcs-policy/src/compose.rs
- `composition()` --calls--> `compose()`  [INFERRED]
  crates/jcs-policy/src/render.rs → crates/jcs-policy/src/compose.rs

## Import Cycles
- 1-file cycle: `crates/jcs-app/src/daemon.rs -> crates/jcs-app/src/daemon.rs`

## Communities (180 total, 76 thin omitted)

### Community 0 - "src/store.rs"
Cohesion: 0.13
Nodes (5): append_event(), encode_timestamp(), encode_ts(), insert_event(), is_unique_violation()

### Community 1 - "OpenCodeClient"
Cohesion: 0.14
Nodes (10): AgentSession, api_errors_surface_the_server_message(), extract_error_message(), into_list(), ModelInfo, OpenCodeClient, PromptAck, truncate() (+2 more)

### Community 3 - "Task"
Cohesion: 0.11
Nodes (8): duration_requires_an_end(), only_process_failures_are_retryable(), sample_run(), Task, RunStatus, task_columns!, Store, TaskRow

### Community 4 - "commit.rs"
Cohesion: 0.12
Nodes (13): builder_rejects_a_bad_summary(), builder_renders_header_and_body(), CommitKind, Chore, Ci, Docs, Feat, Fix (+5 more)

### Community 6 - "http.rs"
Cohesion: 0.08
Nodes (24): a_directory_that_is_not_a_repository_is_rejected(), a_planning_failure_blocks_the_goal_instead_of_leaving_it_pending(), a_registered_repository_appears_in_the_list(), a_relative_project_path_is_rejected_with_a_reason(), ask(), asking_for_an_unknown_project_suggests_the_next_step(), fail_planning(), first_line() (+16 more)

### Community 7 - "render.rs"
Cohesion: 0.22
Nodes (10): duration(), durations_are_human_scaled(), goal_detail(), plural(), plurals_read_naturally(), run_detail(), short_id(), short_ids_drop_the_prefix_and_bound_the_length() (+2 more)

### Community 8 - "ApiError"
Cohesion: 0.10
Nodes (16): AgentError, Api, Decode, Discovery, Io, Json, Stream, Transport (+8 more)

### Community 9 - "status.rs"
Cohesion: 0.21
Nodes (4): any_goal(), any_run(), any_task(), status_enum!

### Community 11 - "jobs"
Cohesion: 0.08
Nodes (30): name, runs-on, steps, name, runs-on, steps, env, CARGO_TERM_COLOR (+22 more)

### Community 12 - "event.rs"
Cohesion: 0.14
Nodes (6): builder_sets_scopes_and_payload(), Event, EVENT_SCHEMA_VERSION, event_kinds!, NewEvent, timestamps_encode_with_fixed_precision_utc()

### Community 13 - "sse.rs"
Cohesion: 0.21
Nodes (16): a_trailing_lone_cr_waits_for_the_next_chunk(), accepts_crlf_and_bare_cr_line_endings(), blank_lines_between_comment_only_blocks_do_not_dispatch(), captures_event_and_id_fields_when_present(), data_without_space_after_colon_is_valid(), decode_all(), decodes_a_minimal_data_only_event(), find_line_end() (+8 more)

### Community 14 - "resolve_project"
Cohesion: 0.12
Nodes (17): compile_policy(), events(), EventsQuery, file_state_label(), gate_views(), gates(), memory_index(), memory_search() (+9 more)

### Community 15 - "Json"
Cohesion: 0.15
Nodes (22): ApiResult, abort(), catalog_models(), event_history(), health(), HistoryQuery, list_goals(), list_projects() (+14 more)

### Community 16 - "languages.rs"
Cohesion: 0.20
Nodes (29): callee_name(), callee_names_are_reduced_to_the_last_segment(), collect_imports(), extract(), extract_python(), extract_rust(), extract_rust_fixture(), extract_sql() (+21 more)

### Community 19 - "jcs-cli/src/main.rs"
Cohesion: 0.15
Nodes (24): ask(), AskArgs, Cli, display_command(), doctor(), follow_goal(), gates(), GatesCommand (+16 more)

### Community 20 - "commit.rs"
Cohesion: 0.21
Nodes (16): a_well_formed_message_passes(), branch_name(), branch_names_carry_the_goal_and_a_task_suffix(), missing_separator_is_rejected(), overlong_summaries_are_rejected(), scope_is_optional_and_may_contain_dashes(), slugify(), slugs_are_ascii_bounded_and_dash_separated() (+8 more)

### Community 22 - "parser.rs"
Cohesion: 0.12
Nodes (32): Finding, Severity, Error, Info, Warning, ansi_sequences_are_stripped_before_parsing(), CARGO_ERROR, cargo_parser_extracts_errors_and_locations() (+24 more)

### Community 23 - "Command"
Cohesion: 0.09
Nodes (22): Command, Abort, Ask, Doctor, Gates, Goal, Index, Logs (+14 more)

### Community 24 - "jcs-git/src/lib.rs"
Cohesion: 0.31
Nodes (4): render_command(), Result, CommitOutcome, Worktree

### Community 25 - "worktrees.rs"
Cohesion: 0.23
Nodes (11): a_dirty_worktree_is_reported(), a_dirty_worktree_needs_force_to_be_discarded(), a_repository_without_commits_is_rejected(), a_worktree_isolates_changes_until_merge(), commit_and_branch_state_are_readable(), creating_a_worktree_over_an_existing_path_is_refused(), fixture(), git() (+3 more)

### Community 26 - "JC Studio"
Cohesion: 0.12
Nodes (16): App de escritorio, Arquitectura, Cómo termina un run, Desarrollo, Estado, Instalación, JC Studio, Licencia (+8 more)

### Community 27 - "path"
Cohesion: 0.08
Nodes (25): prepare_socket(), restrict_permissions(), serve(), serve_connection(), a_second_daemon_refuses_to_steal_a_live_socket(), a_stale_socket_from_a_killed_daemon_is_replaced(), Daemon, get_json() (+17 more)

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
Cohesion: 0.13
Nodes (27): a_valid_plan_passes(), cycles_are_rejected_with_the_chain(), detect_cycle(), duplicate_and_invalid_keys_are_rejected(), empty_and_oversized_plans_are_rejected(), empty_titles_and_descriptions_are_rejected(), path_claims_cannot_escape_or_claim_the_world(), Plan (+19 more)

### Community 45 - "loader.rs"
Cohesion: 0.15
Nodes (24): a_missing_instruction_file_is_an_error(), built_in_packs_load_and_validate(), BUILTIN, builtin_packs(), compute_hash(), disk_packs_override_built_ins_by_id(), Embedded, instruction_files_cannot_escape_the_pack_directory() (+16 more)

### Community 49 - "StatusMachine"
Cohesion: 0.14
Nodes (9): failed_task_can_only_retry_or_cancel(), GateStatus, StatusMachine, transition_error_names_the_entity_and_both_states(), GateResult, GateResultRow, run_columns!, RunRow (+1 more)

### Community 50 - "jcs-proto/src/lib.rs"
Cohesion: 0.08
Nodes (29): AbortRequest, AgentHealth, ErrorBody, errors_carry_a_code_and_a_message(), EventDto, GateOutcomeView, GateRunResponse, GateView (+21 more)

### Community 51 - "GateStatus"
Cohesion: 0.15
Nodes (6): evaluate(), Evaluation, run_all(), GateOutcome, GateSet, ScanOptions

### Community 52 - "chaos.sh"
Cohesion: 0.24
Nodes (10): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, chaos.sh script, start_daemon(), step() (+2 more)

### Community 53 - "e2e.sh"
Cohesion: 0.25
Nodes (9): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e.sh script, step(), wait_for_daemon() (+1 more)

### Community 54 - "App"
Cohesion: 0.09
Nodes (18): App, shift(), status_style(), an_empty_dashboard_renders_every_pane(), centered_fixed(), draw(), draw_details(), draw_goals() (+10 more)

### Community 56 - "Store"
Cohesion: 0.22
Nodes (8): count_events(), events_for_task(), events_since(), latest_event_ts(), latest_seq(), recent_events(), row_to_event(), decode_ts()

### Community 57 - "jcs-policy/src/model.rs"
Cohesion: 0.17
Nodes (15): compose(), gate_sources_are_recorded(), instructions_follow_pack_order_and_keep_provenance(), later_packs_override_earlier_rules(), Overrides, project_overrides_win_over_every_pack(), rules_and_gates_can_be_disabled(), rust_policy() (+7 more)

### Community 58 - "jcs-policy/src/lib.rs"
Cohesion: 0.25
Nodes (9): a_policy_round_trips(), an_invalid_boundary_is_rejected_with_a_reason(), boundary_rules_are_parsed_and_validated(), ProjectPolicy, read(), secret_scanning_is_enabled_by_default_and_can_be_scoped(), SecretsPolicy, unknown_keys_are_rejected_to_catch_typos() (+1 more)

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
Cohesion: 0.10
Nodes (10): CreateSession, data_envelope_is_unwrapped_but_bare_payloads_survive(), EventStream, lists_are_extracted_from_both_shapes(), model_reference_parses_and_roundtrips(), ModelRef, REQUEST_TIMEOUT, ServerInfo (+2 more)

### Community 63 - "planner.rs"
Cohesion: 0.17
Nodes (18): assistant_text(), assistant_text_reads_the_newest_message_and_ignores_reasoning(), braces_inside_strings_do_not_close_the_object(), cancel_forms(), collect_response(), escaped_quotes_do_not_confuse_the_scanner(), extract_json_object(), extracts_bare_json() (+10 more)

### Community 64 - "jcs-agent/src/events.rs"
Cohesion: 0.16
Nodes (15): AgentEvent, decodes_the_real_captured_execution_event(), decodes_the_real_captured_usage_event(), ExecutionOutcome, Failed, Interrupted, Succeeded, extracts_text_deltas_only_from_delta_events() (+7 more)

### Community 65 - "jcs-policy/src/lib.rs"
Cohesion: 0.14
Nodes (14): ApplyReport, FileReport, FileState, Created, Drifted, Missing, Refused, Unchanged (+6 more)

### Community 66 - "Project"
Cohesion: 0.18
Nodes (11): Project, cleanup(), ensure_integration_branch(), integration_worktree(), integration_worktree_path(), MERGE_LEASE_TTL, merge_task(), suffix() (+3 more)

### Community 67 - "search.rs"
Cohesion: 0.15
Nodes (17): SearchHit, chunk_by_id(), fallback(), fts_queries_are_quoted_and_bounded(), fts_queries_drop_punctuation_that_would_break_syntax(), fts_query(), graph(), GRAPH_LIMIT (+9 more)

### Community 68 - "scheduler.rs"
Cohesion: 0.21
Nodes (16): an_empty_claim_conflicts_with_everything_that_is_running(), block_goal(), budget_block(), claim_conflicts_are_detected_across_tasks(), claims_conflict(), conflict_with(), DAY_MS, dispatch() (+8 more)

### Community 69 - "e2e-policy.sh"
Cohesion: 0.18
Nodes (11): fail(), JCS_DB, JCS_DEFAULT_PACKS, JCS_MAX_CONCURRENT, JCS_MAX_REPAIR_CYCLES, JCS_PACKS_DIR, JCS_SOCKET, RUST_LOG (+3 more)

### Community 70 - "jcs-core/src/lib.rs"
Cohesion: 0.10
Nodes (12): CoreError, EmptyField, InvalidId, InvalidTransition, InvalidValue, MAX_PLAN_TASKS, TaskRole, Docs (+4 more)

### Community 71 - "Studio"
Cohesion: 0.05
Nodes (52): AddProjectForm, budgets_must_be_positive_numbers(), ConsoleTab, Events, Terminals, empty_budget_means_no_ceiling(), GlobalForm, number() (+44 more)

### Community 72 - ".pool"
Cohesion: 0.23
Nodes (5): fixture(), integration_branch_is_stored_on_the_goal(), merge_failures_are_recorded_without_losing_the_task(), only_succeeded_unmerged_tasks_are_pending_merge(), Store

### Community 73 - "app.rs"
Cohesion: 0.15
Nodes (15): events_update_the_local_projection(), Focus, Goals, Projects, Tasks, format_event(), goal(), goals_are_filtered_by_the_selected_project() (+7 more)

### Community 74 - "leases.rs"
Cohesion: 0.22
Nodes (7): a_lease_can_be_taken_only_once(), an_expired_lease_is_reclaimed(), expiring_leases_is_batched_and_audited(), fixture(), Lease, only_the_holder_can_renew_or_release(), Store

### Community 75 - "indexer.rs"
Cohesion: 0.19
Nodes (17): Embeddings, Result, clear_project(), embed_chunks(), file_order_is_deterministic(), index_file(), index_project(), irrelevant_and_hidden_directories_are_skipped() (+9 more)

### Community 76 - "Embeddings"
Cohesion: 0.10
Nodes (19): cosine(), cosine_identifies_identical_and_orthogonal_vectors(), decode_vector(), EmbeddingFormat, Ollama, OpenAi, encode_vector(), mismatched_dimensions_are_not_compared() (+11 more)

### Community 77 - "context.rs"
Cohesion: 0.22
Nodes (9): a_hit_renders_with_a_citation_and_the_right_fence(), build(), ContextOptions, ContextPackage, hit(), language_fence(), render_hit(), unknown_extensions_get_a_plain_fence() (+1 more)

### Community 78 - "0007 — Packs en TOML, composición determinista y bucle de reparación"
Cohesion: 0.33
Nodes (6): 0007 — Packs en TOML, composición determinista y bucle de reparación, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 79 - "Language"
Cohesion: 0.13
Nodes (11): HitSource, Fallback, Graph, Lexical, Semantic, Language, JavaScript, Python (+3 more)

### Community 81 - "SymbolKind"
Cohesion: 0.13
Nodes (13): SymbolKind, Class, Constant, Enum, Function, Index, Interface, Module (+5 more)

### Community 82 - "executor.rs"
Cohesion: 0.07
Nodes (41): block_for_input(), build_prompt(), cancel_pending_forms(), commit_message(), commit_messages_are_always_valid(), decide(), describe_form(), fail_task() (+33 more)

### Community 89 - "goal_detail"
Cohesion: 0.40
Nodes (3): goal_detail(), goal_view(), task_view()

### Community 90 - "Store"
Cohesion: 0.16
Nodes (3): Run, Store, Store

### Community 92 - "reviewer.rs"
Cohesion: 0.13
Nodes (23): a_huge_diff_is_truncated_with_a_note(), a_valid_verdict_is_parsed(), a_warning_only_verdict_does_not_block(), findings_are_capped(), findings_must_cite_a_real_location(), MAX_ATTEMPTS, MAX_DIFF_CHARS, parse_verdict() (+15 more)

### Community 95 - "chunk.rs"
Cohesion: 0.36
Nodes (12): a_file_without_symbols_still_yields_a_header_chunk(), a_header_comment_before_the_first_symbol_is_chunked(), a_symbol_starting_on_line_one_gets_no_empty_header(), chunk_file(), chunk_hashes_change_with_content_and_position(), chunking_is_deterministic(), chunks_are_aligned_to_symbols_and_contained_symbols_are_skipped(), oversized_symbols_are_split_with_overlap() (+4 more)

### Community 96 - "Update"
Cohesion: 0.06
Nodes (43): Command, Abort, AddProject, Ask, IndexMemory, LoadSettings, MemoryStats, Policy (+35 more)

### Community 97 - "path"
Cohesion: 0.17
Nodes (9): Edge, EdgeKind, Call, Import, References, Tests, MAX_CHUNK_LINES, MAX_CHUNKS_PER_FILE (+1 more)

### Community 99 - "Goal"
Cohesion: 0.15
Nodes (9): Goal, GoalStatus, promote(), require_reviews(), finalize_goal(), GoalRow, GoalSettings, goal_columns! (+1 more)

### Community 100 - "terminal.rs"
Cohesion: 0.10
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

### Community 104 - "RuntimeSettings"
Cohesion: 0.08
Nodes (25): Config, env_non_empty(), env_parse(), a_patch_only_touches_what_it_names(), an_empty_string_clears_a_model(), concurrency_needs_a_restart_but_is_saved(), config(), environment_is_the_initial_value() (+17 more)

### Community 105 - "telegram.rs"
Cohesion: 0.11
Nodes (15): AppState, ask_text(), goal_text(), handle_command(), help_text(), long_messages_are_truncated(), notify_events(), poll_updates() (+7 more)

### Community 106 - "e2e-memory.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-memory.sh script, step(), XDG_DATA_HOME

### Community 107 - "secrets.rs"
Cohesion: 0.13
Nodes (24): allow_prefixes_suppress_whole_directories(), BENIGN_MARKERS, binary_and_oversized_files_are_skipped(), candidates(), entropy(), ENTROPY_MIN_LEN, ENTROPY_THRESHOLD, entropy_threshold_separates_random_from_words() (+16 more)

### Community 108 - "Command"
Cohesion: 0.25
Nodes (11): ask_requires_both_parts(), Command, Ask, Goal, Help, Projects, Resume, Status (+3 more)

### Community 109 - "State"
Cohesion: 0.10
Nodes (15): ActivityLine, Connection, Connecting, Offline, Online, errors_show_reason_and_hint(), event(), events_keep_the_goal_they_belong_to() (+7 more)

### Community 110 - "0008 — Orquestador durable: plan, DAG, leases y rama de integración"
Cohesion: 0.33
Nodes (6): 0008 — Orquestador durable: plan, DAG, leases y rama de integración, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 112 - "plans.rs"
Cohesion: 0.15
Nodes (10): TaskStatus, a_failed_dependency_blocks_its_dependents_forever(), a_plan_materializes_into_tasks_with_dependencies(), an_invalid_plan_is_refused_before_any_row_is_written(), counts_describe_whether_a_goal_settled(), fixture(), only_tasks_with_satisfied_dependencies_are_ready(), spec() (+2 more)

### Community 113 - "0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado"
Cohesion: 0.33
Nodes (6): 0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 114 - "ParserKind"
Cohesion: 0.33
Nodes (6): ParserKind, Cargo, Coverage, ExitCode, Generic, Node

### Community 118 - "steps"
Cohesion: 0.09
Nodes (22): jobs, linux, name, runs-on, steps, name, on, push (+14 more)

### Community 124 - "ProjectCommand"
Cohesion: 0.40
Nodes (5): ProjectCommand, Add, List, Remove, Settings

### Community 127 - "tui/mod.rs"
Cohesion: 0.13
Nodes (11): Action, Ask, RenderPolicy, ResumeGoal, event_loop(), execute(), FRAME, handle_key() (+3 more)

### Community 128 - "runner.rs"
Cohesion: 0.26
Nodes (16): a_failing_command_fails_with_its_exit_code(), a_missing_program_is_an_error_not_a_pass(), a_successful_command_passes(), a_timeout_kills_the_child(), a_timeout_kills_the_whole_process_group(), both_streams_are_captured(), coverage_can_fail_a_command_that_exited_zero(), cwd() (+8 more)

### Community 129 - "jcs-gates/src/model.rs"
Cohesion: 0.17
Nodes (12): coverage_threshold_only_makes_sense_with_the_coverage_parser(), empty_commands_are_rejected(), ensure_directory(), gate(), GateDef, GateError, Invalid, Io (+4 more)

### Community 130 - "HANDOFF — leer esto primero"
Cohesion: 0.11
Nodes (18): 5.1 Revisor con rúbrica — HECHO, 5.2 Escaneo de secretos — HECHO, 5.3 Fitness functions arquitectónicas — HECHO, 5.3 Revisor con rúbrica (modelo), 5.4 Enrutado por riesgo — HECHO, 5.4b Enrutado por riesgo (histórico), Cómo cerrar una sesión de trabajo, Entorno (+10 more)

### Community 131 - "jcs-memory/src/boundaries.rs"
Cohesion: 0.21
Nodes (19): absolute_and_escaping_paths_are_rejected(), BoundaryRule, empty_and_self_referential_rules_are_rejected(), relative_prefixes_are_accepted(), rule(), a_clean_dependency_direction_passes(), a_forbidden_import_is_reported_with_its_line(), check() (+11 more)

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

### Community 140 - "memory_service.rs"
Cohesion: 0.27
Nodes (4): index_project(), IndexGuard, spawn_index(), spawn_sweeper()

### Community 141 - "EventKind"
Cohesion: 0.15
Nodes (8): EventKind, Args, init_tracing(), main(), restrict_dir(), shutdown_signal(), Result, ProjectRow

### Community 142 - "Runbook"
Cohesion: 0.20
Nodes (10): Comprobación, Configuración, Instalación, La app de escritorio, Nada de esto toca tu rama sin permiso, Operación diaria, Problemas frecuentes, Recuperación (+2 more)

### Community 145 - "0010 — Cliente de escritorio egui con terminales embebidas"
Cohesion: 0.33
Nodes (6): 0010 — Cliente de escritorio egui con terminales embebidas, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

## Knowledge Gaps
- **220 isolated node(s):** `Events`, `Terminals`, `Goals`, `Details`, `Policy` (+215 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 776 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **76 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppState` connect `telegram.rs` to `OpenCodeClient`, `http.rs`, `memory_service.rs`, `EventKind`, `resolve_project`, `Json`, `path`, `loader.rs`, `GateStatus`, `planner.rs`, `Project`, `search.rs`, `scheduler.rs`, `indexer.rs`, `executor.rs`, `goal_detail`, `reviewer.rs`, `Goal`, `ProjectView`, `RuntimeSettings`?**
  _High betweenness centrality (0.048) - this node is a cross-community bridge._
- **What connects `Events`, `Terminals`, `Goals` to the rest of the system?**
  _220 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `src/store.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.13450292397660818 - nodes in this community are weakly interconnected._
- **Why does `Client` connect `Client` to `Update`, `jcs-client/src/lib.rs`, `.post`, `jcs-cli/src/main.rs`, `tui/mod.rs`?**
  _High betweenness centrality (0.042) - this node is a cross-community bridge._
- **Should `OpenCodeClient` be split into smaller, more focused modules?**
  _Cohesion score 0.13763440860215054 - nodes in this community are weakly interconnected._
- **Why does `EventKind` connect `EventKind` to `Task`, `tests/store.rs`, `http.rs`, `render.rs`, `event.rs`, `jcs-cli/src/main.rs`, `StatusMachine`, `Store`, `Project`, `scheduler.rs`, `jcs-core/src/lib.rs`, `.pool`, `app.rs`, `leases.rs`, `executor.rs`, `Goal`, `telegram.rs`, `State`, `plans.rs`?**
  _High betweenness centrality (0.037) - this node is a cross-community bridge._
- **Should `Repo` be split into smaller, more focused modules?**
  _Cohesion score 0.11764705882352941 - nodes in this community are weakly interconnected._