const BACKEND_CI_WORKFLOW: &str = include_str!("../../.github/workflows/backend-ci.yml");
const E2E_SMOKE_WORKFLOW: &str = include_str!("../../.github/workflows/e2e-smoke.yml");
const RUST_MAIN_SOURCE: &str = include_str!("main.rs");
const BACKGROUND_TASKS_SOURCE: &str = include_str!("api/background_tasks.rs");
const BACKGROUND_TASK_RECOVERY_SOURCE: &str = include_str!("tasks/recovery.rs");
const FRONTEND_BACKGROUND_TASK_TYPES_SOURCE: &str =
    include_str!("../../frontend/src/services/modules/backgroundTaskTypes.ts");
const PRODUCTION_READINESS_SERVICE_SOURCE: &str =
    include_str!("services/production_readiness_service.rs");

fn assert_occurs_at_least(document: &str, fragment: &str, expected: usize) {
    let actual = document.matches(fragment).count();
    assert!(
        actual >= expected,
        "expected {fragment:?} at least {expected} times, found {actual}"
    );
}

fn assert_fragments_in_order(document: &str, workflow: &str, fragments: &[&str]) {
    let mut cursor = 0;
    for fragment in fragments {
        let relative_position = document[cursor..].find(fragment).unwrap_or_else(|| {
            panic!("{workflow} is missing ordered fragment {fragment:?} after byte {cursor}")
        });
        cursor += relative_position + fragment.len();
    }
}

fn brace_delta_ignoring_quoted_strings(line: &str) -> isize {
    let mut delta = 0;
    let mut in_string = false;
    let mut escaped = false;

    for character in line.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }

    delta
}

fn extract_top_level_match_string_patterns(
    source: &str,
    match_marker: &str,
    fallback_marker: &str,
) -> Vec<String> {
    let (_, after_match_marker) = source
        .split_once(match_marker)
        .unwrap_or_else(|| panic!("missing match marker {match_marker:?}"));
    let (known_arms, _) = after_match_marker
        .split_once(fallback_marker)
        .unwrap_or_else(|| panic!("missing fallback marker {fallback_marker:?}"));

    let mut task_types = Vec::new();
    let mut arm_header = String::new();
    let mut brace_depth = 0_isize;

    for line in known_arms.lines() {
        if brace_depth == 0 {
            arm_header.push_str(line.trim());
            arm_header.push(' ');

            if let Some((pattern, _)) = arm_header.split_once("=>") {
                let literals: Vec<_> = pattern
                    .split('"')
                    .enumerate()
                    .filter_map(|(index, part)| (index % 2 == 1).then_some(part))
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect();
                assert!(
                    !literals.is_empty(),
                    "background task match arm must use explicit string literals: {pattern:?}"
                );
                task_types.extend(literals);
                arm_header.clear();
            }
        }

        brace_depth += brace_delta_ignoring_quoted_strings(line);
        assert!(
            brace_depth >= 0,
            "background task match braces became unbalanced"
        );
    }

    assert_eq!(
        brace_depth, 0,
        "background task match braces are unbalanced"
    );
    task_types
}

fn extract_single_quoted_union_literals(
    source: &str,
    type_marker: &str,
    end_marker: &str,
) -> Vec<String> {
    let (_, after_type_marker) = source
        .split_once(type_marker)
        .unwrap_or_else(|| panic!("missing type marker {type_marker:?}"));
    let (union_body, _) = after_type_marker
        .split_once(end_marker)
        .unwrap_or_else(|| panic!("missing union end marker {end_marker:?}"));

    union_body
        .split('\'')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 1).then_some(part))
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn workflows_trigger_for_rust_backend_changes() {
    assert_occurs_at_least(BACKEND_CI_WORKFLOW, r#"- "backend-rs/**""#, 2);
    assert_occurs_at_least(E2E_SMOKE_WORKFLOW, r#"- "backend-rs/**""#, 2);
    assert!(BACKEND_CI_WORKFLOW.contains(r#"- ".github/workflows/backend-ci.yml""#));
    assert!(E2E_SMOKE_WORKFLOW.contains(r#"- ".github/workflows/e2e-smoke.yml""#));
}

#[test]
fn backend_ci_keeps_rust_quality_gates_in_order() {
    assert!(BACKEND_CI_WORKFLOW.contains("rust-production:"));
    assert!(BACKEND_CI_WORKFLOW.contains("toolchain: \"1.88\""));
    assert!(BACKEND_CI_WORKFLOW.contains("components: rustfmt, clippy"));
    assert_fragments_in_order(
        BACKEND_CI_WORKFLOW,
        "backend-ci",
        &[
            "cargo fmt --manifest-path backend-rs/Cargo.toml -- --check",
            "cargo check --locked --manifest-path backend-rs/Cargo.toml",
            "cargo test --locked --manifest-path backend-rs/Cargo.toml",
            "cargo clippy --locked --manifest-path backend-rs/Cargo.toml",
            "-D clippy::correctness",
            "-D clippy::suspicious",
        ],
    );
}

#[test]
fn backend_ci_keeps_python_in_migration_support_role() {
    assert!(BACKEND_CI_WORKFLOW.contains("python-migration-support:"));
    assert!(BACKEND_CI_WORKFLOW.contains("Install migration support dependencies"));
    assert!(BACKEND_CI_WORKFLOW.contains("Run Python migration and support regressions"));
    assert!(BACKEND_CI_WORKFLOW.contains("backend/requirements-migrator.txt"));
    assert!(BACKEND_CI_WORKFLOW.contains("-r requirements-migrator.txt"));
    assert!(BACKEND_CI_WORKFLOW.contains("run: pytest tests/test_tools"));
    assert!(!BACKEND_CI_WORKFLOW.contains("-r requirements.txt"));
}

#[test]
fn e2e_smoke_keeps_postgres_rust_and_playwright_execution_order() {
    assert!(E2E_SMOKE_WORKFLOW.contains("rust-real-backend-smoke:"));
    assert_fragments_in_order(
        E2E_SMOKE_WORKFLOW,
        "e2e-smoke",
        &[
            "image: postgres:18-alpine",
            "name: Migrate PostgreSQL with Rust",
            "cargo run --locked -- migration-executor",
            "name: Verify Rust release readiness preflight",
            "cargo run --locked -- release-readiness-preflight",
            "name: Start Rust backend",
            "cargo build --locked",
            "binary_path=\"$(realpath ./target/debug/mumu-novel-backend)\"",
            "binary_sha256=\"$(sha256sum \"$binary_path\" | awk '{print $1}')\"",
            "nohup \"$binary_path\"",
            "name: Wait for Rust backend readiness",
            "http://127.0.0.1:8003/readyz",
            "name: Verify Rust release readiness contract",
            "http://127.0.0.1:8003/releasez",
            "name: Run auth + background-task smoke against Rust",
            "e2e/auth.spec.ts e2e/background-task-pages.spec.ts",
        ],
    );
    assert!(!E2E_SMOKE_WORKFLOW.contains("http://127.0.0.1:8003/health"));
}

#[test]
fn e2e_smoke_preserves_migration_executor_evidence_and_structured_stdout() {
    let step_start = E2E_SMOKE_WORKFLOW
        .find("      - name: Migrate PostgreSQL with Rust")
        .expect("Rust migration executor workflow step");
    let step_end = E2E_SMOKE_WORKFLOW[step_start..]
        .find("      - name: Verify Rust release readiness preflight")
        .map(|offset| step_start + offset)
        .expect("release readiness preflight step after migration");
    let step = &E2E_SMOKE_WORKFLOW[step_start..step_end];

    assert_fragments_in_order(
        step,
        "Rust migration executor workflow step",
        &[
            "working-directory: backend-rs",
            "mkdir -p ../e2e-diagnostics",
            "set +e",
            "cargo run --locked -- migration-executor",
            "> ../e2e-diagnostics/migration-executor.json",
            "2> ../e2e-diagnostics/migration-executor-stderr.log",
            "migration_exit_code=$?",
            "set -e",
            "> ../e2e-diagnostics/migration-executor-exit-code.txt",
            "cat ../e2e-diagnostics/migration-executor.json",
            "cat ../e2e-diagnostics/migration-executor-stderr.log >&2",
            "exit \"$migration_exit_code\"",
        ],
    );
    assert!(!step.contains("migration-executor || true"));

    assert_fragments_in_order(
        RUST_MAIN_SOURCE,
        "machine-readable migration executor CLI",
        &[
            "let structured_cli_output = matches!(",
            "Some(MIGRATION_EXECUTOR_COMMAND | RELEASE_READINESS_PREFLIGHT_COMMAND)",
            "init_tracing(structured_cli_output);",
            "async fn run_migration_executor_command() -> i32",
            "serde_json::to_string_pretty(&report.to_json())",
            "println!(\"{payload}\")",
        ],
    );
}

#[test]
fn e2e_smoke_runs_release_preflight_before_server_and_preserves_artifacts() {
    let step_start = E2E_SMOKE_WORKFLOW
        .find("      - name: Verify Rust release readiness preflight")
        .expect("release readiness preflight workflow step");
    let step_end = E2E_SMOKE_WORKFLOW[step_start..]
        .find("      - name: Start Rust backend")
        .map(|offset| step_start + offset)
        .expect("Rust backend start step after preflight");
    let step = &E2E_SMOKE_WORKFLOW[step_start..step_end];

    assert_fragments_in_order(
        step,
        "release readiness preflight workflow step",
        &[
            "working-directory: backend-rs",
            "mkdir -p ../e2e-diagnostics",
            "set +e",
            "cargo run --locked -- release-readiness-preflight",
            "> ../e2e-diagnostics/release-preflight.json",
            "2> ../e2e-diagnostics/release-preflight-stderr.log",
            "preflight_exit_code=$?",
            "set -e",
            "> ../e2e-diagnostics/release-preflight-exit-code.txt",
            "cat ../e2e-diagnostics/release-preflight.json",
            "cat ../e2e-diagnostics/release-preflight-stderr.log >&2",
            "exit \"$preflight_exit_code\"",
        ],
    );
    assert!(!step.contains("release-readiness-preflight || true"));
}

#[test]
fn e2e_smoke_requires_rust_owned_release_gate_before_playwright() {
    assert_fragments_in_order(
        E2E_SMOKE_WORKFLOW,
        "Rust-owned R0.1 release gate",
        &[
            "name: Wait for Rust backend readiness",
            "name: Verify Rust release readiness contract",
            "releasez_url=\"http://127.0.0.1:8003/releasez\"",
            "releasez_body=\"e2e-diagnostics/releasez.json\"",
            "releasez_status_file=\"e2e-diagnostics/releasez-http-status.txt\"",
            "Rust release readiness contract verified",
            "name: Run auth + background-task smoke against Rust",
        ],
    );
    assert!(!E2E_SMOKE_WORKFLOW.contains("matches_target_storage_contract === true"));
    assert!(!E2E_SMOKE_WORKFLOW.contains("target_storage_contract === 'unbounded_text'"));
    assert!(!E2E_SMOKE_WORKFLOW.contains("node <<'NODE'"));
}

#[test]
fn e2e_smoke_preserves_structured_readyz_failure_diagnostics() {
    assert_fragments_in_order(
        E2E_SMOKE_WORKFLOW,
        "readyz diagnostics",
        &[
            "mkdir -p ../e2e-diagnostics",
            "../e2e-diagnostics/rust-backend.log",
            "readyz_body=\"e2e-diagnostics/readyz.json\"",
            "readyz_status_file=\"e2e-diagnostics/readyz-http-status.txt\"",
            "-o \"$readyz_body\" -w \"%{http_code}\"",
            "Last /readyz response:",
            "cat \"$readyz_body\" || true",
            "releasez_body=\"e2e-diagnostics/releasez.json\"",
            "releasez_status_file=\"e2e-diagnostics/releasez-http-status.txt\"",
            "Last /releasez response:",
            "cat \"$releasez_body\" || true",
            "name: Upload Rust readiness diagnostics",
            "name: rust-readiness-diagnostics",
            "path: e2e-diagnostics/",
        ],
    );
    assert!(!E2E_SMOKE_WORKFLOW.contains("http://127.0.0.1:8003/readyz\" > /dev/null"));
    assert!(!E2E_SMOKE_WORKFLOW.contains("http://127.0.0.1:8003/releasez\" > /dev/null"));
}

#[test]
fn e2e_smoke_persists_successful_runner_evidence_and_always_uploads_diagnostics() {
    assert_fragments_in_order(
        E2E_SMOKE_WORKFLOW,
        "successful Rust E2E evidence",
        &[
            "name: Run auth + background-task smoke against Rust",
            "playwright-smoke.log",
            "playwright-smoke-exit-code.txt",
            "name: Stop Rust backend and record lifecycle",
            "rust-backend-lifecycle.json",
            r#""identity_status": "verified""#,
            r#""cleanup_status": "terminated""#,
            "name: Record successful Rust E2E evidence",
            "if: success()",
            "runner-success.json",
            r#""evidence_status": "passed""#,
            r#""runtime_owner": "backend-rs""#,
            r#""database": "postgresql""#,
            r#""migration_executor": "passed""#,
            r#""release_readiness_preflight": "passed""#,
            r#""readyz": "passed""#,
            r#""releasez": "passed""#,
            r#""playwright_smoke": "passed""#,
            r#""backend_identity": "verified""#,
            r#""backend_lifecycle": "passed""#,
            r#""binary_path": "${binary_path}""#,
            r#""binary_sha256": "${binary_sha256}""#,
            r#""github_sha": "${GITHUB_SHA}""#,
            r#""github_run_id": "${GITHUB_RUN_ID}""#,
            r#""github_run_attempt": "${GITHUB_RUN_ATTEMPT}""#,
            "name: Upload Rust readiness diagnostics",
        ],
    );

    let upload_start = E2E_SMOKE_WORKFLOW
        .find("      - name: Upload Rust readiness diagnostics")
        .expect("Rust E2E evidence upload step");
    let upload_end = E2E_SMOKE_WORKFLOW[upload_start..]
        .find("      - name: Upload Playwright report")
        .map(|offset| upload_start + offset)
        .expect("Playwright artifact step after Rust evidence upload");
    let upload_step = &E2E_SMOKE_WORKFLOW[upload_start..upload_end];

    assert_fragments_in_order(
        upload_step,
        "Rust E2E evidence upload step",
        &[
            "if: always()",
            "uses: actions/upload-artifact@v4",
            "name: rust-readiness-diagnostics",
            "path: e2e-diagnostics/",
        ],
    );
    assert!(!upload_step.contains("if: failure()"));
}

#[test]
fn e2e_smoke_verifies_linux_binary_identity_before_startup_and_cleanup_signals() {
    assert_fragments_in_order(
        E2E_SMOKE_WORKFLOW,
        "Linux Rust binary identity evidence",
        &[
            "cargo build --locked",
            "binary_path=\"$(realpath ./target/debug/mumu-novel-backend)\"",
            "binary_sha256=\"$(sha256sum \"$binary_path\" | awk '{print $1}')\"",
            "rust-backend-binary-path.txt",
            "rust-backend-binary-sha256.txt",
            "nohup \"$binary_path\"",
            "printf '%s\\n' \"$backend_pid\" > ../e2e-diagnostics/rust-backend.pid",
            "[ -e \"/proc/$backend_pid/exe\" ]",
            "observed_binary_path=\"$(readlink -f \"/proc/$backend_pid/exe\")\"",
            "observed_binary_sha256=\"$(sha256sum \"/proc/$backend_pid/exe\" | awk '{print $1}')\"",
            "rust-backend-identity.json",
            r#""identity_status": "${identity_status}""#,
            "name: Stop Rust backend and record lifecycle",
            "case \"$backend_pid\" in",
            r#""identity_status": "invalid_pid""#,
            r#""cleanup_status": "signal_refused""#,
            "expected_binary_path=\"$(cat e2e-diagnostics/rust-backend-binary-path.txt)\"",
            "kill -0 \"$backend_pid\"",
            "[ ! -e \"/proc/$backend_pid/exe\" ]",
            "observed_binary_path=\"$(readlink -f \"/proc/$backend_pid/exe\")\"",
            "observed_binary_sha256=\"$(sha256sum \"/proc/$backend_pid/exe\" | awk '{print $1}')\"",
            r#""identity_status": "mismatch""#,
            r#""cleanup_status": "signal_refused""#,
            "Refusing to signal PID $backend_pid because process identity does not match",
            "kill -TERM \"$backend_pid\"",
        ],
    );
    assert_occurs_at_least(E2E_SMOKE_WORKFLOW, "/proc/$backend_pid/exe", 6);
}

#[test]
fn e2e_smoke_rejects_python_runtime_and_preserves_failure_diagnostics() {
    for forbidden in ["uvicorn", "alembic-sqlite.ini", "sqlite+aiosqlite"] {
        assert!(
            !E2E_SMOKE_WORKFLOW.contains(forbidden),
            "e2e-smoke must not contain legacy runtime fragment {forbidden:?}"
        );
    }

    assert_fragments_in_order(
        E2E_SMOKE_WORKFLOW,
        "direct Rust server lifecycle and failure diagnostics",
        &[
            "cargo build --locked",
            "nohup \"$binary_path\"",
            "name: Run auth + background-task smoke against Rust",
            "set -o pipefail",
            "tee ../e2e-diagnostics/playwright-smoke.log",
            "playwright-smoke-exit-code.txt",
            "name: Stop Rust backend and record lifecycle",
            "if: always()",
            "rust-backend-lifecycle.json",
            r#""cleanup_status": "already_exited""#,
            "exit 1",
            "kill -TERM \"$backend_pid\"",
            r#""cleanup_status": "terminated""#,
            "kill -KILL \"$backend_pid\" || true",
            r#""cleanup_status": "forced_kill""#,
            "name: Record successful Rust E2E evidence",
            "name: Record failed Rust E2E evidence",
            "if: failure()",
            "runner-failure.json",
            r#""evidence_status": "failed""#,
            r#""github_sha": "${GITHUB_SHA}""#,
            r#""github_run_id": "${GITHUB_RUN_ID}""#,
            r#""github_run_attempt": "${GITHUB_RUN_ATTEMPT}""#,
            "name: Upload Rust readiness diagnostics",
            "if: always()",
            "name: Upload Playwright report",
            "if: failure()",
        ],
    );
    assert!(!E2E_SMOKE_WORKFLOW.contains("nohup cargo run --locked"));
    assert!(E2E_SMOKE_WORKFLOW.contains("cat e2e-diagnostics/rust-backend.log || true"));
}

#[test]
fn release_readiness_preflight_is_registered_read_only_and_rust_owned() {
    assert_fragments_in_order(
        RUST_MAIN_SOURCE,
        "release-readiness-preflight command",
        &[
            "const RELEASE_READINESS_PREFLIGHT_COMMAND: &str = \"release-readiness-preflight\";",
            "let structured_cli_output = matches!(",
            "Some(MIGRATION_EXECUTOR_COMMAND | RELEASE_READINESS_PREFLIGHT_COMMAND)",
            "init_tracing(structured_cli_output);",
            "Some(RELEASE_READINESS_PREFLIGHT_COMMAND)",
            "exit(run_release_readiness_preflight_command().await);",
            "async fn run_release_readiness_preflight_command() -> i32",
            "evaluate_production_readiness(Some(&db)).await",
            "evaluation.release_payload()",
        ],
    );

    let function_start = RUST_MAIN_SOURCE
        .find("async fn run_release_readiness_preflight_command() -> i32")
        .expect("release readiness preflight function");
    let function_end = RUST_MAIN_SOURCE[function_start..]
        .find("async fn run_migration_noop_executor_smoke_command() -> i32")
        .map(|offset| function_start + offset)
        .expect("next command function");
    let command_source = &RUST_MAIN_SOURCE[function_start..function_end];

    for forbidden in [
        "run_rust_migration",
        "create_postgres_smoke_schema",
        "execute(Statement",
        "CREATE SCHEMA",
        "ALTER TABLE",
        "DROP SCHEMA",
    ] {
        assert!(
            !command_source.contains(forbidden),
            "release preflight must remain read-only; found {forbidden:?}"
        );
    }

    assert!(RUST_MAIN_SOURCE.contains(".with_writer(std::io::stderr)"));
    assert!(PRODUCTION_READINESS_SERVICE_SOURCE.contains("check_live_alembic_head(conn).await"));
    assert!(PRODUCTION_READINESS_SERVICE_SOURCE
        .contains("check_password_hash_storage_compatibility(conn).await"));
    assert!(PRODUCTION_READINESS_SERVICE_SOURCE
        .contains("password_hash_storage_check.matches_target_storage_contract == Some(true)"));
}

#[test]
fn background_task_startup_persists_orphan_recovery_before_periodic_workers_and_router() {
    assert_fragments_in_order(
        RUST_MAIN_SOURCE,
        "background task startup recovery durability",
        &[
            "let task_registry = tasks::registry::TaskRegistry::new();",
            "tasks::persistence::load_from_disk(&task_registry).await;",
            "let recovered_count = tasks::recovery::recover_orphan_tasks(&task_registry).await;",
            "if recovered_count > 0 {",
            "tasks::persistence::save_to_disk(&task_registry).await;",
            "tasks::persistence::start_periodic_save(task_registry.clone());",
            "start_periodic_cleanup(task_registry.clone());",
            "api::router::build(db, &cfg, task_registry)",
        ],
    );
}

#[test]
fn background_task_startup_recovery_uses_atomic_update_if_owner() {
    let (_, after_function_start) = BACKGROUND_TASK_RECOVERY_SOURCE
        .split_once("async fn recover_orphan_task(")
        .expect("recover_orphan_task function");
    let (function_source, _) = after_function_start
        .split_once("pub async fn recover_orphan_tasks(")
        .expect("recover_orphan_tasks function boundary");

    assert_fragments_in_order(
        function_source,
        "background task startup recovery atomic owner",
        &[
            "let recovered = registry",
            ".update_if(",
            "|task| task.status.is_active()",
            ".await?;",
            "let policy = recovery_policy_for(&recovered.task_type);",
            "Some((recovered.task_id, recovered.task_type, policy))",
        ],
    );
    assert!(
        !function_source.contains(".update("),
        "startup orphan recovery must not bypass update_if with ordinary update"
    );
    assert!(
        !function_source.contains("recovered_metadata"),
        "startup orphan recovery metadata must derive from the update_if result"
    );
}

#[test]
fn generic_background_task_executor_types_have_explicit_recovery_policies() {
    use std::collections::HashSet;

    let task_types = extract_top_level_match_string_patterns(
        BACKGROUND_TASKS_SOURCE,
        "match record.task_type.as_str() {",
        "        other => {",
    );
    let unique_task_types: HashSet<_> = task_types.iter().collect();

    assert_eq!(
        task_types.len(),
        unique_task_types.len(),
        "execute_task must not route the same task type through multiple match arms"
    );
    assert!(
        !task_types.is_empty(),
        "execute_task task type set is empty"
    );

    for task_type in task_types {
        assert!(
            crate::tasks::recovery::has_explicit_recovery_policy(&task_type),
            "generic executable background task {task_type:?} is missing an explicit recovery policy"
        );
    }
}

#[test]
fn frontend_background_task_types_match_rust_execution_and_recovery_owners() {
    use std::collections::HashSet;

    let frontend_task_types = extract_single_quoted_union_literals(
        FRONTEND_BACKGROUND_TASK_TYPES_SOURCE,
        "export type BackgroundTaskType =",
        ";",
    );
    let unique_frontend_task_types: HashSet<_> = frontend_task_types.iter().collect();
    assert_eq!(
        frontend_task_types.len(),
        unique_frontend_task_types.len(),
        "frontend BackgroundTaskType must not contain duplicates"
    );

    let unknown_count = frontend_task_types
        .iter()
        .filter(|task_type| task_type.as_str() == "unknown")
        .count();
    assert_eq!(
        unknown_count, 1,
        "frontend BackgroundTaskType must keep exactly one unknown safety sentinel"
    );
    assert!(
        !crate::tasks::recovery::has_explicit_recovery_policy("unknown"),
        "unknown must keep using the non-resumable fallback instead of an explicit policy"
    );
    assert_eq!(
        crate::tasks::recovery::recovery_policy_for("unknown"),
        crate::tasks::recovery::TaskRecoveryPolicy::NonResumable
    );

    let frontend_known_types: HashSet<_> = frontend_task_types
        .iter()
        .map(String::as_str)
        .filter(|task_type| *task_type != "unknown")
        .collect();
    let recovery_registry_types: HashSet<_> = crate::tasks::recovery::TASK_RECOVERY_POLICIES
        .iter()
        .map(|entry| entry.task_type)
        .collect();
    assert_eq!(
        frontend_known_types, recovery_registry_types,
        "frontend known task types and Rust recovery registry must stay in lockstep"
    );

    let executor_types: HashSet<_> = extract_top_level_match_string_patterns(
        BACKGROUND_TASKS_SOURCE,
        "match record.task_type.as_str() {",
        "        other => {",
    )
    .into_iter()
    .collect();
    let independent_owner_types: HashSet<_> = [
        "chapter_analysis".to_string(),
        "chapter_single_generate".to_string(),
        "chapters_batch_generate".to_string(),
    ]
    .into_iter()
    .collect();
    let non_executor_types: HashSet<_> = frontend_known_types
        .iter()
        .filter(|task_type| !executor_types.contains(**task_type))
        .map(|task_type| (*task_type).to_string())
        .collect();
    assert_eq!(
        non_executor_types, independent_owner_types,
        "frontend types outside execute_task must remain limited to explicit independent owners"
    );
}
