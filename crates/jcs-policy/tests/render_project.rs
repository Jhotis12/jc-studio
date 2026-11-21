//! Project-level policy behaviour: compilation, rendering, drift and overwrite safety.

use jcs_policy::{
    apply, compile, loader::builtin_packs, project, FileState, ProjectPolicy, AGENTS_PATH,
    GATES_PATH, LOCK_PATH,
};

fn packs() -> std::collections::BTreeMap<String, jcs_policy::model::Pack> {
    builtin_packs().unwrap()
}

fn defaults() -> Vec<String> {
    vec!["rust-service".to_string()]
}

#[test]
fn a_project_without_a_policy_file_uses_the_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let compiled = compile(directory.path(), &packs(), &defaults()).unwrap();

    let ids: Vec<String> = compiled.composition.pack_ids();
    assert!(ids.contains(&"rust-service".to_string()));
    assert!(ids.contains(&"core-git".to_string()));
}

#[test]
fn the_project_file_selects_packs_and_overrides_rules() {
    let directory = tempfile::tempdir().unwrap();
    project::write(
        directory.path(),
        &ProjectPolicy {
            packs: vec!["ts-frontend".into()],
            rules: std::collections::BTreeMap::from([(
                "no_unwrap_outside_tests".to_string(),
                "project-specific wording".to_string(),
            )]),
            ..ProjectPolicy::default()
        },
    )
    .unwrap();

    let compiled = compile(directory.path(), &packs(), &defaults()).unwrap();

    let ids = compiled.composition.pack_ids();
    assert!(ids.contains(&"ts-frontend".to_string()));
    assert!(!ids.contains(&"rust-service".to_string()), "{ids:?}");

    let rule = &compiled.composition.rules["no_unwrap_outside_tests"];
    assert_eq!(rule.value, "project-specific wording");
    assert_eq!(rule.source, "project");
}

#[test]
fn apply_creates_then_reports_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let compiled = compile(directory.path(), &packs(), &defaults()).unwrap();

    let first = apply(&compiled, directory.path(), false, false).unwrap();
    assert!(first.ok());
    assert!(first
        .files
        .iter()
        .all(|file| file.state == FileState::Created));
    assert!(directory.path().join(AGENTS_PATH).is_file());
    assert!(directory.path().join(GATES_PATH).is_file());
    assert!(directory.path().join(LOCK_PATH).is_file());

    let second = apply(&compiled, directory.path(), false, false).unwrap();
    assert!(second.ok());
    assert!(second
        .files
        .iter()
        .all(|file| file.state == FileState::Unchanged));
}

#[test]
fn check_passes_right_after_apply_and_fails_after_a_hand_edit() {
    let directory = tempfile::tempdir().unwrap();
    let compiled = compile(directory.path(), &packs(), &defaults()).unwrap();
    apply(&compiled, directory.path(), false, false).unwrap();

    let clean = apply(&compiled, directory.path(), true, false).unwrap();
    assert!(clean.ok(), "{clean:?}");

    let agents = directory.path().join(AGENTS_PATH);
    let mut content = std::fs::read_to_string(&agents).unwrap();
    content.push_str("\nHand-written addition.\n");
    std::fs::write(&agents, content).unwrap();

    let drifted = apply(&compiled, directory.path(), true, false).unwrap();
    assert!(!drifted.ok());
    assert_eq!(drifted.drifted().len(), 1);
    assert_eq!(drifted.drifted()[0].path, AGENTS_PATH);
}

#[test]
fn check_reports_missing_files_before_the_first_apply() {
    let directory = tempfile::tempdir().unwrap();
    let compiled = compile(directory.path(), &packs(), &defaults()).unwrap();

    let report = apply(&compiled, directory.path(), true, false).unwrap();
    assert!(!report.ok());
    assert!(report
        .files
        .iter()
        .all(|file| file.state == FileState::Missing));
}

#[test]
fn a_hand_written_agents_md_is_never_silently_replaced() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join(AGENTS_PATH),
        "# My own instructions\n",
    )
    .unwrap();

    let compiled = compile(directory.path(), &packs(), &defaults()).unwrap();

    let refused = apply(&compiled, directory.path(), false, false).unwrap();
    assert!(!refused.ok());
    assert_eq!(
        refused
            .files
            .iter()
            .find(|file| file.path == AGENTS_PATH)
            .unwrap()
            .state,
        FileState::Refused
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join(AGENTS_PATH)).unwrap(),
        "# My own instructions\n"
    );

    // Other generated files are still written: refusing one file does not block the rest.
    assert!(directory.path().join(GATES_PATH).is_file());
}

#[test]
fn force_replaces_a_hand_written_agents_md_but_keeps_a_backup() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join(AGENTS_PATH),
        "# My own instructions\n",
    )
    .unwrap();

    let compiled = compile(directory.path(), &packs(), &defaults()).unwrap();
    let report = apply(&compiled, directory.path(), false, true).unwrap();

    assert!(report.ok(), "{report:?}");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("AGENTS.md.pre-jcs.bak")).unwrap(),
        "# My own instructions\n"
    );
    assert!(std::fs::read_to_string(directory.path().join(AGENTS_PATH))
        .unwrap()
        .contains("generated by jcs policy render"));
}

#[test]
fn compilation_is_deterministic_across_runs() {
    let directory = tempfile::tempdir().unwrap();
    let first = compile(directory.path(), &packs(), &defaults()).unwrap();
    let second = compile(directory.path(), &packs(), &defaults()).unwrap();

    assert_eq!(first.rendered.lock, second.rendered.lock);
    for (a, b) in first.rendered.files.iter().zip(&second.rendered.files) {
        assert_eq!(a.path, b.path);
        assert_eq!(a.content, b.content);
        assert_eq!(a.sha256, b.sha256);
    }
}

#[test]
fn an_unknown_pack_fails_with_an_actionable_message() {
    let directory = tempfile::tempdir().unwrap();
    project::write(
        directory.path(),
        &ProjectPolicy {
            packs: vec!["does-not-exist".into()],
            ..ProjectPolicy::default()
        },
    )
    .unwrap();

    let error = compile(directory.path(), &packs(), &defaults()).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("does-not-exist"));
    assert!(message.contains("rust-service"));
}
