# Graph Report - jc-studio  (2026-11-21)

## Corpus Check
- 175 files · ~136,477 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 13 file(s) not represented in the graph (top: .toml 8, (none) 3, .desktop 1)

## Summary
- 2782 nodes · 6188 edges · 235 communities (131 shown, 104 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 230 edges (avg confidence: 0.84)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `c55de1c8`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- embeddings.rs
- OpenCodeClient
- Repo
- Task
- commit.rs
- .get
- http.rs
- StoreError
- StoreError
- .pool
- jobs
- src/store.rs
- sse.rs
- scheduler.rs
- Json
- languages.rs
- jcs-cli/src/main.rs
- commit.rs
- parser.rs
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
- plans.rs
- loader.rs
- ui/mod.rs
- jcs-proto/src/lib.rs
- verify.rs
- chaos.sh
- e2e.sh
- ui.rs
- check_contract.py
- .post
- markdown.rs
- jcs-policy/src/lib.rs
- jcs-policy/src/render.rs
- render_project.rs
- Endpoint
- jcs-agent/src/client.rs
- gate_views
- jcs-agent/src/events.rs
- PolicyError
- tui/app.rs
- search.rs
- code.rs
- e2e-policy.sh
- jcs-core/src/model.rs
- src/app.rs
- Event
- planner.rs
- integration.rs
- indexer.rs
- tests/memory.rs
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
- tests/store.rs
- Project
- reviewer.rs
- path
- Update
- dag.rs
- GateStatus
- jcs-policy/src/model.rs
- terminal.rs
- e2e-merge.sh
- e2e-plan.sh
- ProjectView
- RuntimeSettings
- telegram.rs
- e2e-memory.sh
- secrets.rs
- jcs-memory/src/boundaries.rs
- State
- 0008 — Orquestador durable: plan, DAG, leases y rama de integración
- .pool
- 0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado
- ui/settings.rs
- steps
- Command
- jcs-cli/src/render.rs
- tui/mod.rs
- runner.rs
- jcs-gates/src/model.rs
- HANDOFF — leer esto primero
- leases.rs
- FileState
- draw
- soak.sh
- paths.rs
- 0010 — Cliente de escritorio egui con terminales embebidas
- Estado del proyecto
- TaskStage
- connection_loop
- EdgeKind
- Runbook
- 0016 — Markdown en las respuestas y streaming real del Orquestador
- deserialize
- style
- Studio
- goal_detail
- install.sh
- licenses/README.md
- package.sh
- jcs-daemon/src/settings.rs
- jcs-client/src/lib.rs
- Daemon
- status.rs
- trace.rs
- e2e-multigoal.sh
- .goals_for_selected_project
- 0011 — Varios objetivos a la vez, con reparto justo y claims de rutas
- 0012 — Secretos en archivo 0600, fuera de SQLite y de la API
- duration
- jcs-core/src/lib.rs
- 0013 — Progreso por etapas y tareas iterables
- PlanError
- 0014 — Coste calculado desde el catálogo cuando el backend reporta cero
- 0015 — Chat del Orquestador separado de la ejecución de objetivos
- .send
- Pendientes y riesgos aceptados
- SettingsView
- super
- jcs-daemon/src/orchestrator.rs
- extensions.rs
- plan.rs
- CoreError
- 0017 — El gate de secretos mira solo lo que la tarea escribió
- jcs-daemon/src/main.rs
- Config
- Focus
- costs.rs
- Values

## God Nodes (most connected - your core abstractions)
1. `AppState` - 142 edges
2. `Studio` - 85 edges
3. `Client` - 72 edges
4. `Store` - 56 edges
5. `Project` - 55 edges
6. `Command` - 51 edges
7. `Update` - 51 edges
8. `State` - 49 edges
9. `Repo` - 46 edges
10. `ApiResult` - 44 edges

## Surprising Connections (you probably didn't know these)
- `Studio` --references--> `Terminals`  [INFERRED]
  crates/jcs-app/src/app.rs → crates/jcs-app/src/terminal.rs
- `Studio` --references--> `CodeState`  [INFERRED]
  crates/jcs-app/src/app.rs → crates/jcs-app/src/ui/code.rs
- `project_settings()` --calls--> `parse_amount()`  [INFERRED]
  crates/jcs-app/src/ui/settings.rs → crates/jcs-app/src/app.rs
- `project_settings()` --calls--> `parse_tokens()`  [INFERRED]
  crates/jcs-app/src/ui/settings.rs → crates/jcs-app/src/app.rs
- `AppState` --references--> `IndexGuard`  [INFERRED]
  crates/jcs-daemon/src/state.rs → crates/jcs-daemon/src/memory_service.rs

## Import Cycles
- 1-file cycle: `crates/jcs-app/src/daemon.rs -> crates/jcs-app/src/daemon.rs`

## Communities (235 total, 104 thin omitted)

### Community 0 - "embeddings.rs"
Cohesion: 0.11
Nodes (20): cosine(), cosine_identifies_identical_and_orthogonal_vectors(), decode_vector(), EmbeddingFormat, Ollama, OpenAi, encode_vector(), mismatched_dimensions_are_not_compared() (+12 more)

### Community 1 - "OpenCodeClient"
Cohesion: 0.12
Nodes (9): encode_query(), into_list(), McpServerInfo, ModelInfo, OpenCodeClient, PromptAck, SkillInfo, unwrap_data() (+1 more)

### Community 3 - "Task"
Cohesion: 0.14
Nodes (9): Task, TaskStatus, Store, a_blocked_task_can_be_retried_with_feedback(), fixture(), iteration_creates_a_dependent_task_with_feedback(), task_columns!, Store (+1 more)

### Community 4 - "commit.rs"
Cohesion: 0.12
Nodes (13): builder_rejects_a_bad_summary(), builder_renders_header_and_body(), CommitKind, Chore, Ci, Docs, Feat, Fix (+5 more)

### Community 6 - "http.rs"
Cohesion: 0.08
Nodes (34): a_directory_that_is_not_a_repository_is_rejected(), a_finished_task_can_be_iterated_and_a_merge_retried(), a_goal_can_be_cancelled_and_deleted(), a_greeting_is_not_an_objective(), a_planning_failure_blocks_the_goal_instead_of_leaving_it_pending(), a_registered_repository_appears_in_the_list(), a_relative_project_path_is_rejected_with_a_reason(), ask() (+26 more)

### Community 7 - "StoreError"
Cohesion: 0.11
Nodes (10): StoreError, Conflict, Corrupt, Db, Domain, Io, Json, Locked (+2 more)

### Community 8 - "StoreError"
Cohesion: 0.10
Nodes (16): AgentError, Api, Decode, Discovery, Io, Json, Stream, Transport (+8 more)

### Community 9 - ".pool"
Cohesion: 0.13
Nodes (10): Goal, GoalStatus, promote(), require_reviews(), finalize_goal(), deleting_a_goal_removes_tasks_runs_and_leases(), GoalRow, GoalSettings (+2 more)

### Community 11 - "jobs"
Cohesion: 0.08
Nodes (30): name, runs-on, steps, name, runs-on, steps, env, CARGO_TERM_COLOR (+22 more)

### Community 12 - "src/store.rs"
Cohesion: 0.12
Nodes (10): OrchestratorMessage, Store, the_transcript_and_session_are_durable_and_clearable(), append_event(), concurrent_appends_do_not_lock_the_database(), decode_ts(), encode_timestamp(), encode_ts() (+2 more)

### Community 13 - "sse.rs"
Cohesion: 0.21
Nodes (16): a_trailing_lone_cr_waits_for_the_next_chunk(), accepts_crlf_and_bare_cr_line_endings(), blank_lines_between_comment_only_blocks_do_not_dispatch(), captures_event_and_id_fields_when_present(), data_without_space_after_colon_is_valid(), decode_all(), decodes_a_minimal_data_only_event(), find_line_end() (+8 more)

### Community 14 - "scheduler.rs"
Cohesion: 0.18
Nodes (17): an_empty_claim_conflicts_with_everything_that_is_running(), block_goal(), budget_block(), claim_conflicts_are_detected_across_tasks(), claims_conflict(), conflict_with(), DAY_MS, dispatch() (+9 more)

### Community 15 - "Json"
Cohesion: 0.11
Nodes (39): ApiResult, abort(), abort_run(), add_mcp_server(), cancel_goal(), catalog_mcp(), catalog_models(), catalog_skills() (+31 more)

### Community 16 - "languages.rs"
Cohesion: 0.19
Nodes (30): callee_name(), callee_names_are_reduced_to_the_last_segment(), collect_imports(), extract(), extract_python(), extract_rust(), extract_rust_fixture(), extract_sql() (+22 more)

### Community 19 - "jcs-cli/src/main.rs"
Cohesion: 0.14
Nodes (24): ask(), AskArgs, Cli, display_command(), doctor(), follow_goal(), gates(), GatesCommand (+16 more)

### Community 20 - "commit.rs"
Cohesion: 0.21
Nodes (16): a_well_formed_message_passes(), branch_name(), branch_names_carry_the_goal_and_a_task_suffix(), missing_separator_is_rejected(), overlong_summaries_are_rejected(), scope_is_optional_and_may_contain_dashes(), slugify(), slugs_are_ascii_bounded_and_dash_separated() (+8 more)

### Community 22 - "parser.rs"
Cohesion: 0.12
Nodes (32): Finding, Severity, Error, Info, Warning, ansi_sequences_are_stripped_before_parsing(), CARGO_ERROR, cargo_parser_extracts_errors_and_locations() (+24 more)

### Community 23 - "Command"
Cohesion: 0.07
Nodes (27): Command, Abort, Ask, Doctor, Gates, Goal, Index, Logs (+19 more)

### Community 24 - "pathbuf"
Cohesion: 0.21
Nodes (9): render_command(), Result, ChangedText, CommitOutcome, hunk_new_start(), MAX_UNTRACKED_BYTES, parse_added_lines(), strip_diff_prefix() (+1 more)

### Community 25 - "worktrees.rs"
Cohesion: 0.22
Nodes (12): a_dirty_worktree_is_reported(), a_dirty_worktree_needs_force_to_be_discarded(), a_repository_without_commits_is_rejected(), a_worktree_isolates_changes_until_merge(), commit_and_branch_state_are_readable(), creating_a_worktree_over_an_existing_path_is_refused(), fixture(), git() (+4 more)

### Community 26 - "JC Studio"
Cohesion: 0.12
Nodes (16): App de escritorio, Arquitectura, Cómo termina un run, Desarrollo, Estado, Instalación, JC Studio, Licencia (+8 more)

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
Cohesion: 0.18
Nodes (9): a_failed_dependency_blocks_its_dependents_forever(), a_plan_materializes_into_tasks_with_dependencies(), an_invalid_plan_is_refused_before_any_row_is_written(), counts_describe_whether_a_goal_settled(), fixture(), only_tasks_with_satisfied_dependencies_are_ready(), spec(), Store (+1 more)

### Community 45 - "loader.rs"
Cohesion: 0.18
Nodes (24): a_missing_instruction_file_is_an_error(), built_in_packs_load_and_validate(), BUILTIN, builtin_packs(), compute_hash(), disk_packs_override_built_ins_by_id(), Embedded, instruction_files_cannot_escape_the_pack_directory() (+16 more)

### Community 49 - "ui/mod.rs"
Cohesion: 0.20
Nodes (16): Notice, TaskProgress, central(), connection_badge(), console(), details(), events(), model_selector() (+8 more)

### Community 50 - "jcs-proto/src/lib.rs"
Cohesion: 0.06
Nodes (39): identifiers_are_shortened_for_display(), short(), run_detail(), run_view(), run_views_expose_cost_and_tokens(), AbortRequest, AgentHealth, ErrorBody (+31 more)

### Community 52 - "chaos.sh"
Cohesion: 0.24
Nodes (10): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, chaos.sh script, start_daemon(), step() (+2 more)

### Community 53 - "e2e.sh"
Cohesion: 0.25
Nodes (9): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e.sh script, step(), wait_for_daemon() (+1 more)

### Community 54 - "ui.rs"
Cohesion: 0.12
Nodes (4): an_empty_dashboard_renders_every_pane(), rendered(), the_input_modal_is_rendered_when_open(), the_selected_goal_and_its_tasks_are_visible()

### Community 56 - ".post"
Cohesion: 0.08
Nodes (3): run(), Client, encode()

### Community 57 - "markdown.rs"
Cohesion: 0.23
Nodes (14): code_block(), has_closing(), heading(), headings_and_lists_are_recognised(), inline(), is_rule(), is_table_separator(), links_keep_label_and_url() (+6 more)

### Community 58 - "jcs-policy/src/lib.rs"
Cohesion: 0.21
Nodes (10): a_policy_round_trips(), an_invalid_boundary_is_rejected_with_a_reason(), boundary_rules_are_parsed_and_validated(), POLICY_FILE, ProjectPolicy, read(), secret_scanning_is_enabled_by_default_and_can_be_scoped(), SecretsPolicy (+2 more)

### Community 59 - "jcs-policy/src/render.rs"
Cohesion: 0.21
Nodes (20): sha256_hex(), agents_md_carries_the_marker_packs_and_rule_provenance(), changing_a_rule_changes_the_rendered_output(), composition(), content_of(), GateDocument, gates_toml_round_trips_to_the_same_gate_set(), GatesDocument (+12 more)

### Community 60 - "render_project.rs"
Cohesion: 0.29
Nodes (16): apply(), compile(), Compiled, a_hand_written_agents_md_is_never_silently_replaced(), a_project_without_a_policy_file_uses_the_defaults(), an_unknown_pack_fails_with_an_actionable_message(), apply_creates_then_reports_unchanged(), check_passes_right_after_apply_and_fails_after_a_hand_edit() (+8 more)

### Community 61 - "Endpoint"
Cohesion: 0.14
Nodes (7): base_url_loses_trailing_slashes(), debug_never_prints_the_password(), Endpoint, service_password(), service_url(), opencode_config_dir(), Result

### Community 62 - "jcs-agent/src/client.rs"
Cohesion: 0.10
Nodes (14): AgentSession, api_errors_surface_the_server_message(), CreateSession, data_envelope_is_unwrapped_but_bare_payloads_survive(), EventStream, extract_error_message(), lists_are_extracted_from_both_shapes(), model_reference_parses_and_roundtrips() (+6 more)

### Community 63 - "gate_views"
Cohesion: 0.40
Nodes (3): gate_views(), parser_label(), PolicyGateView

### Community 64 - "jcs-agent/src/events.rs"
Cohesion: 0.14
Nodes (16): AgentEvent, decodes_the_real_captured_execution_event(), decodes_the_real_captured_usage_event(), ExecutionOutcome, Failed, Interrupted, Succeeded, extracts_reasoning_deltas_separately_from_text() (+8 more)

### Community 65 - "PolicyError"
Cohesion: 0.17
Nodes (11): PolicyError, Cycle, InvalidGate, InvalidPack, Io, MissingPack, PathEscape, Refused (+3 more)

### Community 66 - "tui/app.rs"
Cohesion: 0.21
Nodes (12): events_update_the_local_projection(), format_event(), goal(), goals_are_filtered_by_the_selected_project(), LOG_LINES, project(), selection_is_clamped_when_the_list_shrinks(), shift() (+4 more)

### Community 67 - "search.rs"
Cohesion: 0.23
Nodes (16): SearchHit, chunk_by_id(), fallback(), fts_queries_are_quoted_and_bounded(), fts_queries_drop_punctuation_that_would_break_syntax(), fts_query(), graph(), GRAPH_LIMIT (+8 more)

### Community 68 - "code.rs"
Cohesion: 0.12
Nodes (23): a_file_is_read_and_saved(), a_manual_commit_from_the_project_root_works(), build_tree(), CodeState, commit_panel(), desired_root(), diff_panel(), editor() (+15 more)

### Community 69 - "e2e-policy.sh"
Cohesion: 0.18
Nodes (11): fail(), JCS_DB, JCS_DEFAULT_PACKS, JCS_MAX_CONCURRENT, JCS_MAX_REPAIR_CYCLES, JCS_PACKS_DIR, JCS_SOCKET, RUST_LOG (+3 more)

### Community 70 - "jcs-core/src/model.rs"
Cohesion: 0.09
Nodes (12): duration_requires_an_end(), only_process_failures_are_retryable(), Run, sample_run(), RunStatus, a_positive_reported_cost_is_kept(), a_zero_reported_cost_is_computed_from_the_stored_rates(), GateResultRow (+4 more)

### Community 71 - "src/app.rs"
Cohesion: 0.07
Nodes (29): AddProjectForm, budgets_must_be_positive_numbers(), ConsoleTab, Agent, Events, Terminals, empty_budget_means_no_ceiling(), GlobalForm (+21 more)

### Community 72 - "Event"
Cohesion: 0.07
Nodes (16): builder_sets_scopes_and_payload(), Event, EVENT_SCHEMA_VERSION, EventKind, event_kinds!, NewEvent, timestamps_encode_with_fixed_precision_utc(), Result (+8 more)

### Community 73 - "planner.rs"
Cohesion: 0.17
Nodes (18): assistant_text(), assistant_text_reads_the_newest_message_and_ignores_reasoning(), braces_inside_strings_do_not_close_the_object(), cancel_forms(), collect_response(), escaped_quotes_do_not_confuse_the_scanner(), extract_json_object(), extracts_bare_json() (+10 more)

### Community 74 - "integration.rs"
Cohesion: 0.21
Nodes (5): fixture(), integration_branch_is_stored_on_the_goal(), merge_failures_are_recorded_without_losing_the_task(), only_succeeded_unmerged_tasks_are_pending_merge(), Store

### Community 75 - "indexer.rs"
Cohesion: 0.17
Nodes (19): Embeddings, clear_project(), embed_chunks(), file_order_is_deterministic(), index_file(), index_project(), irrelevant_and_hidden_directories_are_skipped(), persist() (+11 more)

### Community 76 - "tests/memory.rs"
Cohesion: 0.27
Nodes (13): a_second_index_run_skips_everything_unchanged(), an_empty_or_missing_index_returns_no_hits_instead_of_failing(), context_packages_are_cited_and_budgeted(), deleted_files_disappear_from_the_index(), extra_sections_are_included_when_the_budget_allows(), fixture(), indexing_extracts_symbols_chunks_and_sql_tables(), only_changed_files_are_reindexed() (+5 more)

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
Cohesion: 0.18
Nodes (13): task_progress(), card(), composer(), goals(), duration(), goal(), task_card(), panel() (+5 more)

### Community 81 - "SymbolKind"
Cohesion: 0.13
Nodes (13): SymbolKind, Class, Constant, Enum, Function, Index, Interface, Module (+5 more)

### Community 82 - "executor.rs"
Cohesion: 0.08
Nodes (44): block_for_input(), build_prompt(), cancel_pending_forms(), commit_message(), commit_messages_are_always_valid(), decide(), describe_form(), fail_task() (+36 more)

### Community 89 - "tests/store.rs"
Cohesion: 0.25
Nodes (13): an_empty_gate_set_is_not_a_pass(), backup_produces_a_readable_snapshot(), crashed_runs_become_interrupted_and_retryable(), deleting_a_project_cascades_state_but_preserves_the_audit_trail(), event_log_is_monotonic_and_resumable(), fixture(), goal_and_task_transitions_are_enforced_by_the_store(), orphaned_tasks_are_parked_as_blocked_not_failed() (+5 more)

### Community 90 - "Project"
Cohesion: 0.20
Nodes (15): Project, an_equal_integration_branch_is_not_stale(), cleanup(), ensure_integration_branch(), git(), integration_is_stale(), integration_worktree(), integration_worktree_path() (+7 more)

### Community 92 - "reviewer.rs"
Cohesion: 0.11
Nodes (23): a_huge_diff_is_truncated_with_a_note(), a_valid_verdict_is_parsed(), a_warning_only_verdict_does_not_block(), findings_are_capped(), findings_must_cite_a_real_location(), MAX_ATTEMPTS, MAX_DIFF_CHARS, parse_verdict() (+15 more)

### Community 95 - "path"
Cohesion: 0.36
Nodes (12): a_file_without_symbols_still_yields_a_header_chunk(), a_header_comment_before_the_first_symbol_is_chunked(), a_symbol_starting_on_line_one_gets_no_empty_header(), chunk_file(), chunk_hashes_change_with_content_and_position(), chunking_is_deterministic(), chunks_are_aligned_to_symbols_and_contained_symbols_are_skipped(), oversized_symbols_are_split_with_overlap() (+4 more)

### Community 96 - "Update"
Cohesion: 0.13
Nodes (20): handle(), Update, Event, Gates, GoalAction, GoalDeleted, Mcp, MemoryIndex (+12 more)

### Community 97 - "dag.rs"
Cohesion: 0.16
Nodes (8): GAP_X, GAP_Y, graph(), level(), NODE_HEIGHT, NODE_WIDTH, PAD, truncate()

### Community 98 - "GateStatus"
Cohesion: 0.21
Nodes (5): failed_task_can_only_retry_or_cancel(), GateStatus, StatusMachine, transition_error_names_the_entity_and_both_states(), GateResult

### Community 99 - "jcs-policy/src/model.rs"
Cohesion: 0.17
Nodes (15): compose(), gate_sources_are_recorded(), instructions_follow_pack_order_and_keep_provenance(), later_packs_override_earlier_rules(), Overrides, project_overrides_win_over_every_pack(), rules_and_gates_can_be_disabled(), rust_policy() (+7 more)

### Community 100 - "terminal.rs"
Cohesion: 0.08
Nodes (15): a_command_runs_and_reaches_the_screen(), color(), nonzero(), render(), Session, Terminals, the_256_colour_cube_maps_to_rgb(), a_second_daemon_refuses_to_steal_a_live_socket() (+7 more)

### Community 101 - "e2e-merge.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-merge.sh script, step(), XDG_DATA_HOME

### Community 102 - "e2e-plan.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-plan.sh script, step(), XDG_DATA_HOME

### Community 103 - "ProjectView"
Cohesion: 0.40
Nodes (5): add_project(), project_view(), update_project(), AddProjectRequest, ProjectResponse

### Community 105 - "telegram.rs"
Cohesion: 0.13
Nodes (25): ask_requires_both_parts(), ask_text(), Command, Ask, Goal, Help, Projects, Resume (+17 more)

### Community 106 - "e2e-memory.sh"
Cohesion: 0.24
Nodes (8): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_SOCKET, RUST_LOG, e2e-memory.sh script, step(), XDG_DATA_HOME

### Community 107 - "secrets.rs"
Cohesion: 0.10
Nodes (35): allow_prefixes_suppress_whole_directories(), BENIGN_MARKERS, binary_and_oversized_files_are_skipped(), candidates(), entropy(), ENTROPY_MIN_LEN, ENTROPY_THRESHOLD, entropy_threshold_separates_random_from_words() (+27 more)

### Community 108 - "jcs-memory/src/boundaries.rs"
Cohesion: 0.31
Nodes (15): BoundaryRule, a_clean_dependency_direction_passes(), a_forbidden_import_is_reported_with_its_line(), check(), describe(), files_under(), invalid_rules_are_ignored_instead_of_panicking(), mentions_module() (+7 more)

### Community 109 - "State"
Cohesion: 0.08
Nodes (22): Connection, Connecting, Offline, Online, errors_show_reason_and_hint(), event(), events_keep_the_goal_they_belong_to(), replayed_history_does_not_duplicate_lines() (+14 more)

### Community 110 - "0008 — Orquestador durable: plan, DAG, leases y rama de integración"
Cohesion: 0.33
Nodes (6): 0008 — Orquestador durable: plan, DAG, leases y rama de integración, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 113 - "0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado"
Cohesion: 0.33
Nodes (6): 0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 114 - "ui/settings.rs"
Cohesion: 0.42
Nodes (7): add_project_window(), daemon_settings(), iteration_window(), project_settings(), restart_daemon(), settings_window(), windows()

### Community 118 - "steps"
Cohesion: 0.09
Nodes (22): jobs, linux, name, runs-on, steps, name, on, push (+14 more)

### Community 124 - "Command"
Cohesion: 0.06
Nodes (31): Command, Abort, AddMcp, AddProject, Ask, CancelGoal, ConnectMcp, DeleteGoal (+23 more)

### Community 126 - "jcs-cli/src/render.rs"
Cohesion: 0.22
Nodes (10): duration(), durations_are_human_scaled(), goal_detail(), plural(), plurals_read_naturally(), run_detail(), short_id(), short_ids_drop_the_prefix_and_bound_the_length() (+2 more)

### Community 127 - "tui/mod.rs"
Cohesion: 0.13
Nodes (10): Action, Ask, RenderPolicy, ResumeGoal, event_loop(), execute(), FRAME, handle_key() (+2 more)

### Community 128 - "runner.rs"
Cohesion: 0.28
Nodes (16): a_failing_command_fails_with_its_exit_code(), a_missing_program_is_an_error_not_a_pass(), a_successful_command_passes(), a_timeout_kills_the_child(), a_timeout_kills_the_whole_process_group(), both_streams_are_captured(), coverage_can_fail_a_command_that_exited_zero(), cwd() (+8 more)

### Community 129 - "jcs-gates/src/model.rs"
Cohesion: 0.11
Nodes (21): run_all(), coverage_threshold_only_makes_sense_with_the_coverage_parser(), empty_commands_are_rejected(), ensure_directory(), gate(), GateDef, GateError, Invalid (+13 more)

### Community 130 - "HANDOFF — leer esto primero"
Cohesion: 0.11
Nodes (18): 5.1 Revisor con rúbrica — HECHO, 5.2 Escaneo de secretos — HECHO, 5.3 Fitness functions arquitectónicas — HECHO, 5.3 Revisor con rúbrica (modelo), 5.4 Enrutado por riesgo — HECHO, 5.4b Enrutado por riesgo (histórico), Cómo cerrar una sesión de trabajo, Entorno (+10 more)

### Community 131 - "leases.rs"
Cohesion: 0.22
Nodes (7): a_lease_can_be_taken_only_once(), an_expired_lease_is_reclaimed(), expiring_leases_is_batched_and_audited(), fixture(), Lease, only_the_holder_can_renew_or_release(), Store

### Community 132 - "FileState"
Cohesion: 0.15
Nodes (13): ApplyReport, FileReport, FileState, Created, Drifted, Missing, Refused, Unchanged (+5 more)

### Community 133 - "draw"
Cohesion: 0.30
Nodes (12): status_style(), centered_fixed(), draw(), draw_details(), draw_goals(), draw_input(), draw_log(), draw_projects() (+4 more)

### Community 134 - "soak.sh"
Cohesion: 0.19
Nodes (12): fail(), goal_status(), JCS_DB, JCS_DEFAULT_PACKS, JCS_MAX_CONCURRENT, JCS_MEMORY_SWEEP_SECS, JCS_SOCKET, RUST_LOG (+4 more)

### Community 136 - "paths.rs"
Cohesion: 0.33
Nodes (10): SOCKET_NAME, data_dir(), database_lives_under_the_data_directory(), database_path(), log_dir(), runtime_dir(), socket_lives_under_the_runtime_directory(), socket_path() (+2 more)

### Community 137 - "0010 — Cliente de escritorio egui con terminales embebidas"
Cohesion: 0.24
Nodes (6): 0010 — Cliente de escritorio egui con terminales embebidas, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 138 - "Estado del proyecto"
Cohesion: 0.18
Nodes (11): Avance, Estado del proyecto, Evidencia reproducible, Fase 3: qué falta exactamente, Fase 4: qué falta exactamente, Fase 5: qué falta exactamente, Fase 6: qué falta exactamente, Fase 7: qué falta exactamente (+3 more)

### Community 139 - "TaskStage"
Cohesion: 0.15
Nodes (10): TaskStage, Agent, Blocked, Cancelled, Done, Failed, Integrating, MergeFailed (+2 more)

### Community 140 - "connection_loop"
Cohesion: 0.20
Nodes (10): connection_loop(), notice(), refresh(), History, Models, Offline, Online, Settings (+2 more)

### Community 141 - "EdgeKind"
Cohesion: 0.29
Nodes (5): EdgeKind, Call, Import, References, Tests

### Community 142 - "Runbook"
Cohesion: 0.11
Nodes (18): Chat, skills y MCP, Comprobación, Configuración, Coste, Gestionar objetivos, Instalación, Iterar una tarea, La app de escritorio (+10 more)

### Community 145 - "0016 — Markdown en las respuestas y streaming real del Orquestador"
Cohesion: 0.29
Nodes (6): 0016 — Markdown en las respuestas y streaming real del Orquestador, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 146 - "deserialize"
Cohesion: 0.25
Nodes (6): clearing_the_token_does_not_fall_back_to_the_environment(), Secrets, the_hint_never_contains_the_whole_token(), the_token_is_written_private_and_reloaded(), Values, write_private()

### Community 148 - "Studio"
Cohesion: 0.12
Nodes (5): Studio, windows(), bubble(), panel(), windows()

### Community 150 - "goal_detail"
Cohesion: 0.40
Nodes (4): goal_detail(), goal_view(), task_view(), task_views()

### Community 176 - "jcs-daemon/src/settings.rs"
Cohesion: 0.29
Nodes (10): a_patch_only_touches_what_it_names(), an_empty_string_clears_a_model(), concurrency_needs_a_restart_but_is_saved(), config(), environment_is_the_initial_value(), invalid_ranges_are_rejected_and_leave_the_value_untouched(), set_number(), set_optional() (+2 more)

### Community 179 - "jcs-client/src/lib.rs"
Cohesion: 0.13
Nodes (4): prepare_socket(), restrict_permissions(), serve(), serve_connection()

### Community 182 - "status.rs"
Cohesion: 0.21
Nodes (4): any_goal(), any_run(), any_task(), status_enum!

### Community 184 - "trace.rs"
Cohesion: 0.26
Nodes (15): truncate(), ActivityLine, assistant_message(), coalesce(), duration(), Item, Reasoning, System (+7 more)

### Community 185 - "e2e-multigoal.sh"
Cohesion: 0.22
Nodes (9): fail(), JCS_DB, JCS_MAX_CONCURRENT, JCS_MODEL, JCS_SOCKET, RUST_LOG, e2e-multigoal.sh script, step() (+1 more)

### Community 187 - "0011 — Varios objetivos a la vez, con reparto justo y claims de rutas"
Cohesion: 0.29
Nodes (6): 0011 — Varios objetivos a la vez, con reparto justo y claims de rutas, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 188 - "0012 — Secretos en archivo 0600, fuera de SQLite y de la API"
Cohesion: 0.29
Nodes (6): 0012 — Secretos en archivo 0600, fuera de SQLite y de la API, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 189 - "duration"
Cohesion: 0.31
Nodes (4): index_project(), IndexGuard, spawn_index(), spawn_sweeper()

### Community 190 - "jcs-core/src/lib.rs"
Cohesion: 0.19
Nodes (9): MAX_PLAN_TASKS, Plan, TaskRole, Docs, Implement, Migrate, Review, Test (+1 more)

### Community 193 - "0013 — Progreso por etapas y tareas iterables"
Cohesion: 0.29
Nodes (6): 0013 — Progreso por etapas y tareas iterables, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 194 - "PlanError"
Cohesion: 0.15
Nodes (14): detect_cycle(), PlanError, AbsolutePath, Cycle, DuplicateKey, Empty, EmptyDescription, EmptyTitle (+6 more)

### Community 197 - "0014 — Coste calculado desde el catálogo cuando el backend reporta cero"
Cohesion: 0.29
Nodes (6): 0014 — Coste calculado desde el catálogo cuando el backend reporta cero, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 198 - "0015 — Chat del Orquestador separado de la ejecución de objetivos"
Cohesion: 0.29
Nodes (6): 0015 — Chat del Orquestador separado de la ejecución de objetivos, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 204 - "Pendientes y riesgos aceptados"
Cohesion: 0.33
Nodes (6): Coste auxiliar, Decisiones diferidas (no son deuda, son alcance), Limitaciones conocidas, Pendientes y riesgos aceptados, Riesgos aceptados, RUSTSEC-2026-0459 — SQLite empaquetado en `sqlx` 0.9

### Community 205 - "SettingsView"
Cohesion: 0.60
Nodes (5): settings(), settings_view(), update_settings(), SettingsPatch, SettingsView

### Community 209 - "super"
Cohesion: 0.12
Nodes (9): absolute_and_escaping_paths_are_rejected(), empty_and_self_referential_rules_are_rejected(), relative_prefixes_are_accepted(), rule(), claims_overlap(), empty_claims_conflict_with_anything_running(), nested_prefixes_overlap_but_siblings_do_not(), paths_overlap() (+1 more)

### Community 212 - "jcs-daemon/src/orchestrator.rs"
Cohesion: 0.26
Nodes (8): finish(), flush_chunk(), FLUSH_INTERVAL, flush_reasoning(), POLL_INTERVAL, run(), spawn_turn(), TURN_TIMEOUT

### Community 214 - "extensions.rs"
Cohesion: 0.39
Nodes (6): ExtensionsTab, Mcp, Skills, mcp(), panel(), skills()

### Community 217 - "plan.rs"
Cohesion: 0.44
Nodes (11): a_valid_plan_passes(), cycles_are_rejected_with_the_chain(), duplicate_and_invalid_keys_are_rejected(), empty_and_oversized_plans_are_rejected(), empty_titles_and_descriptions_are_rejected(), path_claims_cannot_escape_or_claim_the_world(), plan(), plans_roundtrip_through_json() (+3 more)

### Community 219 - "CoreError"
Cohesion: 0.25
Nodes (5): CoreError, EmptyField, InvalidId, InvalidTransition, InvalidValue

### Community 220 - "0017 — El gate de secretos mira solo lo que la tarea escribió"
Cohesion: 0.29
Nodes (6): 0017 — El gate de secretos mira solo lo que la tarea escribió, Alternativas consideradas, Consecuencias, Contexto, Decisión, Verificación

### Community 221 - "jcs-daemon/src/main.rs"
Cohesion: 0.24
Nodes (5): Args, init_tracing(), main(), restrict_dir(), shutdown_signal()

### Community 222 - "Config"
Cohesion: 0.36
Nodes (4): Config, env_non_empty(), env_parse(), stored_values_win_over_the_environment()

### Community 223 - "Focus"
Cohesion: 0.40
Nodes (4): Focus, Goals, Projects, Tasks

## Knowledge Gaps
- **258 isolated node(s):** `Events`, `Agent`, `Terminals`, `Orchestrator`, `Goals` (+253 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 911 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **104 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppState` connect `Json` to `OpenCodeClient`, `http.rs`, `.pool`, `scheduler.rs`, `deserialize`, `goal_detail`, `loader.rs`, `jcs-proto/src/lib.rs`, `jcs-client/src/lib.rs`, `duration`, `Event`, `planner.rs`, `indexer.rs`, `SettingsView`, `executor.rs`, `jcs-daemon/src/orchestrator.rs`, `Project`, `reviewer.rs`, `jcs-daemon/src/main.rs`, `Config`, `costs.rs`, `ProjectView`, `RuntimeSettings`, `.fmt`, `telegram.rs`?**
  _High betweenness centrality (0.047) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `Studio` (e.g. with `Terminals` and `CodeState`) actually correct?**
  _`Studio` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Events`, `Agent`, `Terminals` to the rest of the system?**
  _258 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `embeddings.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10507246376811594 - nodes in this community are weakly interconnected._
- **Why does `Studio` connect `Studio` to `dag.rs`, `terminal.rs`, `code.rs`, `src/app.rs`, `State`, `dag.rs`, `ui/mod.rs`, `ui/settings.rs`, `Daemon`, `extensions.rs`, `trace.rs`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **Should `OpenCodeClient` be split into smaller, more focused modules?**
  _Cohesion score 0.12312312312312312 - nodes in this community are weakly interconnected._
- **Why does `Store` connect `Event` to `OpenCodeClient`, `search.rs`, `leases.rs`, `Task`, `jcs-core/src/model.rs`, `.pool`, `integration.rs`, `indexer.rs`, `tests/memory.rs`, `context.rs`, `src/store.rs`, `Json`, `plans.rs`, `executor.rs`, `deserialize`, `tests/store.rs`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._