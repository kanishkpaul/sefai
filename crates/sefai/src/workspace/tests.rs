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

#[test]
fn invalid_replacement_preserves_last_good_bytes() {
    let fixture = Fixture::new();
    let original = fs::read(fixture.0.join("config.md")).expect("original");
    fixture.write("snapshot.md", std::str::from_utf8(&original).expect("utf8"));
    assert!(
        fixture
            .workspace()
            .put(
                Path::new("config.md"),
                Some(&fixture.0.join("snapshot.md")),
                "---\nschema: 9\n---\n".as_bytes()
            )
            .is_err()
    );
    assert_eq!(
        fs::read(fixture.0.join("config.md")).expect("retained"),
        original
    );
}

#[test]
fn stale_snapshot_cannot_overwrite_an_external_edit() {
    let fixture = Fixture::new();
    fixture.write("snapshot.md", "---\nschema: 1\n---\n");
    fixture.write("config.md", "---\nschema: 1\n---\nexternal edit\n");
    assert!(
        fixture
            .workspace()
            .put(
                Path::new("config.md"),
                Some(&fixture.0.join("snapshot.md")),
                "---\nschema: 1\n---\nreplacement".as_bytes()
            )
            .expect_err("conflict")
            .to_string()
            .contains("conflict")
    );
    assert!(
        fs::read_to_string(fixture.0.join("config.md"))
            .expect("retained")
            .contains("external edit")
    );
}

#[test]
fn session_body_cannot_inject_a_role() {
    let fixture = Fixture::new();
    let text = "hello\n## assistant\n<!-- end -->\n---\n```\n> nested\n";
    fixture
        .workspace()
        .append("one", "user", text, None)
        .expect("append");
    assert_eq!(
        fixture.workspace().session("one", 4).expect("read"),
        vec![Message::user(text)]
    );
}

#[test]
fn session_tail_is_bounded_in_record_count() {
    let fixture = Fixture::new();
    fixture
        .workspace()
        .append("one", "user", "first", None)
        .expect("create");
    fs::copy(
        fixture.0.join("sessions/one.md"),
        fixture.0.join("snapshot.md"),
    )
    .expect("snapshot");
    fixture
        .workspace()
        .append(
            "one",
            "assistant",
            "second",
            Some(&fixture.0.join("snapshot.md")),
        )
        .expect("append");
    assert_eq!(
        fixture.workspace().session("one", 1).expect("tail"),
        vec![Message::assistant("second")]
    );
}

#[test]
fn incomplete_session_is_rejected_before_replacement() {
    let fixture = Fixture::new();
    let broken = "---\nschema: 1\nid: one\noutcome: interrupted\n---\n## user\n> cut off\n";
    assert!(
        fixture
            .workspace()
            .put(Path::new("sessions/one.md"), None, broken.as_bytes())
            .is_err()
    );
    assert!(!fixture.0.join("sessions/one.md").exists());
}

#[test]
fn workspace_init_preserves_existing_customization() {
    let fixture = Fixture::new();
    fixture.write("system.md", "user's manual edit");
    Workspace::init(&fixture.0).expect("init");
    assert_eq!(
        fs::read_to_string(fixture.0.join("system.md")).expect("read"),
        "user's manual edit"
    );
}

#[test]
fn exact_snapshot_allows_memory_edit_and_forget() {
    let fixture = Fixture::new();
    let workspace = fixture.workspace();
    workspace.remember("one", "first", None).expect("remember");
    let snapshot = fixture.0.join("snapshot.md");
    fs::copy(fixture.0.join("memory/one.md"), &snapshot).expect("snapshot");
    workspace
        .remember("one", "second", Some(&snapshot))
        .expect("edit");
    fs::copy(fixture.0.join("memory/one.md"), &snapshot).expect("new snapshot");
    workspace.forget("one", &snapshot).expect("forget");
    assert!(!fixture.0.join("memory/one.md").exists());
}

#[test]
fn front_matter_is_bounded_before_yaml_parsing() {
    let text = format!(
        "---\nschema: 1\n# {}\n---\n",
        "x".repeat(schema::FRONT_BYTES_MAX)
    );
    assert!(schema::parse::<Config>(&text).is_err());
}

#[test]
fn yaml_aliases_are_rejected() {
    assert!(schema::parse::<Config>("---\nschema: &v 1\nrunner: {threads: *v}\n---\n").is_err());
}

#[test]
fn nested_unknown_fields_report_the_document_path() {
    let fixture = Fixture::new();
    fixture.write(
        "models/one.md",
        "---\nschema: 1\nid: one\npath: one.gguf\nformat: gguf\nrunner:\n  threadz: 3\n---\n",
    );
    let error = fixture
        .workspace()
        .check()
        .expect_err("unknown key")
        .to_string();
    assert!(error.replace('\\', "/").contains("models/one.md") && error.contains("threadz"));
}

#[test]
fn oversize_candidate_preserves_existing_file() {
    let fixture = Fixture::new();
    let snapshot = fixture.0.join("snapshot.md");
    fs::copy(fixture.0.join("config.md"), &snapshot).expect("snapshot");
    let text = format!(
        "---\nschema: 1\n---\n{}",
        "x".repeat(schema::FILE_BYTES_MAX)
    );
    assert!(
        fixture
            .workspace()
            .put(Path::new("config.md"), Some(&snapshot), text.as_bytes())
            .is_err()
    );
    assert_eq!(
        fs::read(fixture.0.join("config.md")).expect("retained"),
        fs::read(snapshot).expect("snapshot")
    );
}

#[test]
fn read_errors_remove_the_temporary_file_and_preserve_the_target() {
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("injected source failure"))
        }
    }
    let fixture = Fixture::new();
    let snapshot = fixture.0.join("snapshot.md");
    fs::copy(fixture.0.join("config.md"), &snapshot).expect("snapshot");
    assert!(
        fixture
            .workspace()
            .put(Path::new("config.md"), Some(&snapshot), Interrupted)
            .is_err()
    );
    assert_eq!(
        fs::read(fixture.0.join("config.md")).expect("retained"),
        fs::read(snapshot).expect("snapshot")
    );
    assert!(!fs::read_dir(&fixture.0).expect("directory").any(|entry| {
        entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
}

#[test]
fn edit_during_staging_is_detected_before_rename() {
    struct EditingSource {
        target: PathBuf,
        source: io::Cursor<&'static [u8]>,
    }
    impl Read for EditingSource {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            fs::write(&self.target, "---\nschema: 1\n---\nexternal edit")?;
            self.source.read(buffer)
        }
    }
    let fixture = Fixture::new();
    let target = fixture.0.join("config.md");
    let snapshot = fixture.0.join("snapshot.md");
    fs::copy(&target, &snapshot).expect("snapshot");
    let source = EditingSource {
        target: target.clone(),
        source: io::Cursor::new(b"---\nschema: 1\n---\ncandidate"),
    };
    assert!(
        fixture
            .workspace()
            .put(Path::new("config.md"), Some(&snapshot), source)
            .is_err()
    );
    assert!(
        fs::read_to_string(target)
            .expect("external edit")
            .ends_with("external edit")
    );
}

#[test]
fn session_tail_is_bounded_in_decoded_bytes() {
    let fixture = Fixture::new();
    let body = "x".repeat(700_000);
    let record = session::encode("user", &body).expect("encode");
    let text = format!("---\nschema: 1\nid: one\noutcome: completed\n---\n{record}{record}");
    fixture.write("sessions/one.md", &text);
    let messages = fixture
        .workspace()
        .session("one", 128)
        .expect("stream tail");
    assert_eq!(messages, vec![Message::user(body)]);
}

#[test]
fn full_size_single_line_session_record_round_trips() {
    let body = "x".repeat(schema::FILE_BYTES_MAX);
    let record = session::encode("user", &body).expect("encode");
    let text = format!("---\nschema: 1\nid: one\noutcome: completed\n---\n{record}");
    let (_, messages) = session::read(io::Cursor::new(text), 1).expect("read maximum record");
    assert_eq!(messages[0].content, body);
}

#[test]
fn crlf_session_records_normalize_line_endings() {
    let text = "---\r\nschema: 1\r\nid: one\r\noutcome: interrupted\r\n---\r\n## user\r\n> first\r\n> second\r\n<!-- end -->\r\n";
    let (_, messages) = session::read(io::Cursor::new(text), 1).expect("CRLF");
    assert_eq!(messages, vec![Message::user("first\nsecond")]);
}

#[test]
fn every_incomplete_record_prefix_is_rejected() {
    let header = "---\nschema: 1\nid: one\noutcome: interrupted\n---\n";
    let record = session::encode("user", "text\n## assistant").expect("encode");
    for offset in 1..record.len() {
        assert!(
            session::read(io::Cursor::new(format!("{header}{}", &record[..offset])), 1).is_err(),
            "accepted prefix {offset}"
        );
    }
}

#[test]
fn crash_leftover_temporary_files_are_never_loaded() {
    let fixture = Fixture::new();
    fixture.write("memory/.sefai-terminated.tmp", "malformed partial bytes");
    assert!(fixture.workspace().check().is_ok());
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected_for_reading() {
    let workspace = Fixture::new();
    let outside = Fixture::new();
    outside.write("system.md", "---\nschema: 1\n---\nexternal text");
    std::os::unix::fs::symlink(outside.0.join("system.md"), workspace.0.join("system.md"))
        .expect("symlink");
    assert!(
        workspace
            .selection()
            .prepare("hi".into())
            .err()
            .expect("escape")
            .to_string()
            .contains("escapes workspace")
    );
}

#[cfg(unix)]
#[test]
fn symlink_parent_is_rejected_for_writing() {
    let workspace = Fixture::new();
    let outside = Fixture::new();
    std::os::unix::fs::symlink(&outside.0, workspace.0.join("memory")).expect("symlink");
    assert!(workspace.workspace().remember("one", "text", None).is_err());
    assert!(!outside.0.join("one.md").exists());
}

#[test]
fn traversal_resource_is_rejected() {
    let fixture = Fixture::new();
    fixture.write(
        "skills/one/SKILL.md",
        "---\nschema: 1\nid: one\nresources: [../../../outside.md]\n---\n",
    );
    assert!(fixture.workspace().check().is_err());
}

#[test]
fn windows_reserved_ids_are_rejected_on_all_platforms() {
    for id in ["con", "nul", "com1", "lpt9", "aux", "prn"] {
        assert!(schema::id(id).is_err());
    }
}

#[test]
fn prompt_budget_rejects_a_section_before_copying_it() {
    let mut prompt = Prompt::new(vec![Message::user("user")]);
    let text = "x".repeat(PROMPT_BYTES_MAX);
    assert!(add_section(&mut prompt, "skill", &text).is_err());
    assert!(prompt.sections().is_empty());
}

#[test]
fn resumed_history_and_user_text_share_one_prompt_budget() {
    let fixture = Fixture::new();
    fixture
        .workspace()
        .append("one", "user", &"x".repeat(700_000), None)
        .expect("persist");
    let mut selection = fixture.selection();
    selection.session = Some("one".into());
    assert!(selection.prepare("y".repeat(700_000)).is_err());
}

#[test]
fn profile_selection_overrides_config_model() {
    let fixture = Fixture::new();
    fixture.write("config.md", "---\nschema: 1\nmodel: one\n---\n");
    for id in ["one", "two"] {
        fixture.write(
            &format!("models/{id}.md"),
            &format!("---\nschema: 1\nid: {id}\npath: {id}.gguf\nformat: gguf\n---\n"),
        );
    }
    let mut selection = fixture.selection();
    selection.profile = Some("two".into());
    assert_eq!(
        selection.prepare("hi".into()).expect("profile").model,
        Some(
            fixture
                .0
                .canonicalize()
                .expect("canonical workspace")
                .join("two.gguf")
        )
    );
}

#[test]
fn unsupported_engine_profiles_never_fall_back_to_gguf() {
    let fixture = Fixture::new();
    fixture.write(
        "models/one.md",
        "---\nschema: 1\nid: one\npath: weights\nformat: mlx\n---\n",
    );
    let mut selection = fixture.selection();
    selection.profile = Some("one".into());
    assert!(
        selection
            .prepare("hi".into())
            .err()
            .expect("unsupported engine")
            .to_string()
            .contains("only gguf")
    );
}

#[test]
fn unsupported_yaml_tags_are_rejected() {
    assert!(schema::parse::<Config>("---\nschema: !custom 1\n---\n").is_err());
}

#[test]
fn the_target_itself_cannot_be_used_as_a_revision_snapshot() {
    let fixture = Fixture::new();
    let target = fixture.0.join("config.md");
    assert!(
        fixture
            .workspace()
            .put(
                Path::new("config.md"),
                Some(&target),
                "---\nschema: 1\n---\nreplacement".as_bytes()
            )
            .is_err()
    );
}

#[test]
fn full_size_crlf_record_has_the_same_decoded_limit() {
    let body = "x".repeat(schema::FILE_BYTES_MAX);
    let record = session::encode("user", &body)
        .expect("encode")
        .replace('\n', "\r\n");
    let text = format!("---\r\nschema: 1\r\nid: one\r\noutcome: completed\r\n---\r\n{record}");
    let (_, messages) = session::read(io::Cursor::new(text), 1).expect("maximum CRLF record");
    assert_eq!(messages[0].content, body);
}
