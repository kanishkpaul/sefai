use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "sefai-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("unique test directory");
        let fixture = Self(path);
        fixture.write("config.md", "---\nschema: 1\n---\n");
        fixture
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
        fs::write(path, text).expect("write fixture");
    }
    fn selection(&self) -> Selection {
        Selection {
            workspace: Some(self.0.clone()),
            ..Selection::default()
        }
    }
    fn workspace(&self) -> Workspace {
        Workspace::open(&self.0).expect("open fixture")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unknown_configuration_fields_are_rejected() {
    assert!(schema::parse::<Config>("---\nschema: 1\ncontex: 12\n---\n").is_err());
}

#[test]
fn duplicate_yaml_fields_are_rejected() {
    assert!(schema::parse::<Config>("---\nschema: 1\nschema: 1\n---\n").is_err());
}

#[test]
fn body_is_never_configuration() {
    let (config, body) = schema::parse::<Config>("---\nschema: 1\n---\nrunner:\n  threads: 999\n")
        .expect("valid prose");
    assert!(config.runner.threads.is_none());
    assert!(body.contains("999"));
}

#[test]
fn unsupported_schema_reports_file_and_field() {
    let fixture = Fixture::new();
    fixture.write("config.md", "---\nschema: 2\n---\n");
    let error = fixture
        .workspace()
        .check()
        .expect_err("future version")
        .to_string();
    assert!(error.contains("config.md") && error.contains("schema"));
}

#[test]
fn runner_ranges_are_checked_without_loading_a_model() {
    let fixture = Fixture::new();
    fixture.write(
        "config.md",
        "---\nschema: 1\nrunner:\n  threads: 257\n---\n",
    );
    assert!(
        fixture
            .workspace()
            .check()
            .expect_err("invalid range")
            .to_string()
            .contains("runner.threads")
    );
}

#[test]
fn instructions_have_deterministic_precedence() {
    let global = Fixture::new();
    global.write("system.md", "---\nschema: 1\n---\nglobal\n");
    let local = Fixture::new();
    local.write("system.md", "---\nschema: 1\n---\nlocal\n");
    local.write("preferences.md", "---\nschema: 1\n---\npreferences\n");
    let mut selection = local.selection();
    selection.global_workspace = Some(global.0.clone());
    let result = selection.prepare("hi".into()).expect("compose");
    assert_eq!(
        result.prompt.to_messages()[0].content,
        "global\n\nlocal\n\npreferences"
    );
}

#[test]
fn unselected_skill_is_not_read() {
    let fixture = Fixture::new();
    fixture.write("skills/broken/SKILL.md", "invalid");
    assert!(fixture.selection().prepare("hi".into()).is_ok());
}

#[test]
fn profile_overrides_workspace_runner_values() {
    let fixture = Fixture::new();
    fixture.write(
        "config.md",
        "---\nschema: 1\nmodel: small\nrunner:\n  threads: 2\n---\n",
    );
    fixture.write(
        "models/small.md",
        "---\nschema: 1\nid: small\npath: weights.gguf\nformat: gguf\nrunner:\n  threads: 3\n---\n",
    );
    let result = fixture.selection().prepare("hi".into()).expect("compose");
    assert_eq!(result.runner.threads, Some(3));
}

#[test]
fn mismatched_ids_are_rejected() {
    let fixture = Fixture::new();
    fixture.write("memory/one.md", "---\nschema: 1\nid: two\n---\n");
    assert!(fixture.workspace().check().is_err());
}

#[test]
fn recall_uses_sorted_files_and_a_count_limit() {
    let fixture = Fixture::new();
    for index in (0..10).rev() {
        fixture.write(
            &format!("memory/m{index}.md"),
            &format!("---\nschema: 1\nid: m{index}\n---\nmatch {index}"),
        );
    }
    let mut selection = fixture.selection();
    selection.recall = Some("match".into());
    let result = selection.prepare("hi".into()).expect("recall");
    assert_eq!(result.prompt.sections().len(), 8);
    assert_eq!(result.prompt.sections()[0].text, "match 0");
}
