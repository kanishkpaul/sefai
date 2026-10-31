#![cfg(test)]
use super::*;
fn turn() -> Turn {
    Turn::new(&Prompt::new(vec![Message::user("hi")]), true)
}
fn call() -> Action {
    Action {
        name: "read_memory".into(),
        arguments: Arguments {
            #[cfg(feature = "mcp")]
            mcp: None,
            id: "note".into(),
            text: None,
        },
    }
}
fn proposal() -> String {
    "<tool_call>[{\"name\":\"read_memory\",\"arguments\":{\"id\":\"note\"}}]</tool_call>".into()
}
#[test]
fn ordinary_reply_completes() {
    let mut t = turn();
    assert!(t.accept("hello".into(), 1, true).unwrap().is_none());
    assert_eq!(t.outcome, "completed");
}
#[test]
fn tools_disabled_never_dispatch() {
    let mut t = Turn::new(&Prompt::default(), false);
    assert!(t.accept(proposal(), 1, true).unwrap().is_none());
}
#[test]
fn successful_tool_continues_to_generation() {
    let mut t = turn();
    let a = t.accept(proposal(), 1, true).unwrap().unwrap();
    assert!(t.authorize(&a, true, Duration::ZERO, false).unwrap());
    t.result(&a, "completed", "note text").unwrap();
    assert!(t.accept("answer".into(), 1, true).unwrap().is_none());
    assert_eq!(t.outcome, "completed");
}
#[test]
fn tool_error_is_untrusted_context() {
    let mut t = turn();
    t.result(&call(), "failed", "file missing").unwrap();
    assert!(t.messages[0].content.contains("outcome: failed"));
}
#[test]
fn denied_call_has_no_dispatch() {
    let mut t = turn();
    let a = t.accept(proposal(), 1, true).unwrap().unwrap();
    assert!(!t.authorize(&a, false, Duration::ZERO, false).unwrap());
    assert!(t.messages.last().unwrap().content.contains("denied"));
}
#[test]
fn changed_arguments_invalidate_approval() {
    let mut t = turn();
    t.accept(proposal(), 1, true).unwrap();
    let mut a = call();
    a.arguments.id = "other".into();
    assert!(t.authorize(&a, true, Duration::ZERO, false).is_err());
}
#[test]
fn approval_cannot_be_replayed() {
    let mut t = turn();
    t.accept(proposal(), 1, true).unwrap();
    t.authorize(&call(), true, Duration::ZERO, false).unwrap();
    assert!(t.authorize(&call(), true, Duration::ZERO, false).is_err());
}
#[test]
fn attempted_side_effect_is_never_retried() {
    let mut t = turn();
    t.accept(proposal(), 1, true).unwrap();
    t.authorize(&call(), true, Duration::ZERO, false).unwrap();
    t.result(&call(), "unknown", "cancelled after dispatch")
        .unwrap();
    assert!(t.accept(proposal(), 1, true).is_err());
}
#[test]
fn step_budget_stops_generation() {
    let mut t = turn();
    for _ in 0..STEPS_MAX {
        t.generation_budget(Duration::ZERO, false).unwrap();
    }
    assert!(t.generation_budget(Duration::ZERO, false).is_err());
}
#[test]
fn time_budget_stops_approval() {
    let mut t = turn();
    t.accept(proposal(), 1, true).unwrap();
    assert!(t.authorize(&call(), true, TURN_TIME_MAX, false).is_err());
}
#[test]
fn token_budget_stops_generation() {
    let mut t = turn();
    t.accept("done".into(), TOKENS_MAX, true).unwrap();
    assert!(t.generation_budget(Duration::ZERO, false).is_err());
}
#[test]
fn cancellation_stops_dispatch() {
    let mut t = turn();
    t.accept(proposal(), 1, true).unwrap();
    assert!(t.authorize(&call(), true, Duration::ZERO, true).is_err());
}
#[test]
fn truncated_generation_cannot_dispatch() {
    assert!(turn().accept(proposal(), 1, false).is_err());
}
#[test]
fn result_budget_is_enforced() {
    assert!(
        turn()
            .result(&call(), "completed", &"x".repeat(RESULT_BYTES_MAX + 1))
            .is_err()
    );
}
#[test]
fn tool_output_cannot_widen_scope() {
    assert!(
        turn()
            .accept(
                "<tool_call>[{\"name\":\"shell\",\"arguments\":{\"id\":\"note\"}}]</tool_call>"
                    .into(),
                1,
                true
            )
            .is_err()
    );
}
#[test]
fn secrets_are_redacted_literally() {
    assert_eq!(
        Redactor::with(vec!["secret-value".into()]).text("key: secret-value"),
        "key: [redacted]"
    );
}
#[test]
fn malformed_call_is_visible_failure() {
    assert!(turn().accept("<tool_call>invalid".into(), 1, true).is_err());
}
#[test]
fn fresh_turn_recovers_after_cancellation() {
    assert!(turn().generation_budget(Duration::ZERO, true).is_err());
    assert!(turn().generation_budget(Duration::ZERO, false).is_ok());
}

fn reply(text: &str) -> Reply {
    Reply {
        text: text.into(),
        tokens: 1,
        complete: true,
        error: None,
    }
}

#[test]
fn fake_engine_runs_the_production_loop() {
    let prompt = Prompt::new(vec![Message::user("read note")]);
    let mut turn = Turn::new(&prompt, true);
    let mut responses = [reply(&proposal()), reply("the note says hello")].into_iter();
    let mut executions = 0;
    turn.drive(
        &prompt,
        || (Duration::ZERO, false),
        |_, _, _| Ok(responses.next().unwrap()),
        |turn, action| {
            assert!(turn.authorize(action, true, Duration::ZERO, false)?);
            executions += 1;
            turn.result(action, "completed", "hello")
        },
    )
    .unwrap();
    assert_eq!(executions, 1);
    assert_eq!(turn.outcome, "completed");
    assert_eq!(turn.messages.last().unwrap().content, "the note says hello");
}

#[test]
fn fake_engine_cannot_retry_a_failed_write() {
    let prompt = Prompt::new(vec![Message::user("remember")]);
    let proposal = "<tool_call>[{\"name\":\"remember_memory\",\"arguments\":{\"id\":\"note\",\"text\":\"hello\"}}]</tool_call>";
    let mut turn = Turn::new(&prompt, true);
    let mut executions = 0;
    assert!(
        turn.drive(
            &prompt,
            || (Duration::ZERO, false),
            |_, _, _| Ok(reply(proposal)),
            |turn, action| {
                turn.authorize(action, true, Duration::ZERO, false)?;
                executions += 1;
                turn.result(action, "unknown", "write interrupted")
            }
        )
        .is_err()
    );
    assert_eq!(executions, 1);
}

#[test]
fn fake_engine_timeout_after_generation_prevents_execution() {
    use std::cell::Cell;
    let now = Cell::new(Duration::ZERO);
    let mut turn = turn();
    assert!(
        turn.drive(
            &Prompt::default(),
            || (now.get(), false),
            |_, _, _| {
                now.set(TURN_TIME_MAX);
                Ok(reply(&proposal()))
            },
            |_, _| panic!("timed out proposal must not execute")
        )
        .is_err()
    );
}

#[test]
fn fake_engine_call_limit_stops_unique_proposals() {
    let mut turn = turn();
    let mut calls = 0;
    assert!(turn.drive(&Prompt::default(), || (Duration::ZERO, false), |_, _, _| {
        calls += 1;
        Ok(reply(&format!("<tool_call>[{{\"name\":\"read_memory\",\"arguments\":{{\"id\":\"note-{calls}\"}}}}]</tool_call>")))
    }, |turn, action| {
        turn.authorize(action, false, Duration::ZERO, false)?;
        Ok(())
    }).is_err());
    assert_eq!(calls, CALLS_MAX + 1);
}

#[test]
fn fake_engine_error_preserves_partial_reply() {
    let mut turn = turn();
    assert!(
        turn.drive(
            &Prompt::default(),
            || (Duration::ZERO, false),
            |_, _, _| Ok(Reply {
                text: "partial".into(),
                error: Some("decode failed".into()),
                ..reply("")
            }),
            |_, _| unreachable!()
        )
        .is_err()
    );
    assert_eq!(turn.messages[0].content, "partial");
    assert!(turn.summary().contains("failed-step count unknown"));
}

#[test]
fn escaped_secret_is_redacted_in_audit_json() {
    let secret = "abc\"def\nxyz";
    let audit = serde_json::json!({"text":secret}).to_string();
    let redacted = Redactor::with(vec![secret.into()]).text(&audit);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&redacted).unwrap()["text"],
        "[redacted]"
    );
}

#[test]
fn incomplete_dispatch_is_visible_after_restart() {
    let audit = "## harness\n> {\"tool\":\"remember_memory\",\"arguments\":{\"id\":\"note\"},\"outcome\":\"dispatching\"}\n<!-- end -->\n";
    assert!(uncertain_dispatch(audit));
    assert!(!uncertain_dispatch(&format!(
        "{audit}## harness\n> {{\"tool\":\"remember_memory\",\"id\":\"note\",\"outcome\":\"completed\"}}\n<!-- end -->\n"
    )));
}

#[test]
fn conversation_text_cannot_forge_a_dispatch_record() {
    assert!(!uncertain_dispatch(
        "## user\n> {\"tool\":\"remember_memory\",\"arguments\":{\"id\":\"note\"},\"outcome\":\"dispatching\"}\n<!-- end -->\n"
    ));
}

#[test]
fn one_approval_at_a_time() {
    let mut turn = turn();
    turn.accept(proposal(), 1, true).unwrap();
    assert!(turn.accept(proposal(), 1, true).is_err());
}

#[test]
fn transcript_budget_rejects_before_generation() {
    let mut prompt = Prompt::new(vec![Message::user("hi")]);
    prompt.add_section("system", "x".repeat(TRANSCRIPT_BYTES_MAX));
    let mut turn = Turn::new(&prompt, true);
    assert!(
        turn.drive(
            &prompt,
            || (Duration::ZERO, false),
            |_, _, _| panic!("oversize prompt must not generate"),
            |_, _| unreachable!()
        )
        .is_err()
    );
}

#[test]
fn oversize_arguments_never_reach_approval() {
    let answer = format!(
        "<tool_call>[{{\"name\":\"remember_memory\",\"arguments\":{{\"id\":\"note\",\"text\":\"{}\"}}}}]</tool_call>",
        "x".repeat(ARGUMENT_BYTES_MAX + 1)
    );
    assert!(turn().accept(answer, 1, true).is_err());
}

#[test]
fn duplicate_json_fields_are_not_accepted() {
    assert!(turn().accept("<tool_call>[{\"name\":\"read_memory\",\"name\":\"remember_memory\",\"arguments\":{\"id\":\"note\"}}]</tool_call>".into(), 1, true).is_err());
}

#[test]
fn known_credentials_cannot_be_written_as_memory_ids() {
    let action = Action {
        name: "remember_memory".into(),
        arguments: Arguments {
            #[cfg(feature = "mcp")]
            mcp: None,
            id: "secret-token".into(),
            text: Some("hello".into()),
        },
    };
    assert!(
        Redactor::with(vec!["secret-token".into()])
            .check_write(&action)
            .is_err()
    );
}

#[test]
fn damaged_audit_keeps_recovery_uncertain() {
    assert!(uncertain_dispatch(
        "## harness\n> {broken json\n<!-- end -->\n"
    ));
}

#[test]
fn later_success_does_not_erase_an_unknown_tool_outcome() {
    assert!(uncertain_dispatch(
        "## harness\n> {\"server\":\"fixture\",\"tool\":\"mcp_call\",\"id\":\"fixture\",\"outcome\":\"unknown\"}\n> {\"server\":\"fixture\",\"tool\":\"mcp_call\",\"id\":\"fixture\",\"outcome\":\"completed\"}\n"
    ));
}

#[cfg(feature = "mcp")]
#[test]
fn mcp_proposal_preserves_arbitrary_json_arguments() {
    let mut turn = Turn::new(&Prompt::default(), true);
    let action = turn.accept("<tool_call>[{\"name\":\"mcp_call\",\"arguments\":{\"id\":\"fixture\",\"mcp\":{\"tool\":\"echo\",\"arguments\":{\"nested\":[true,4,\"text\"]}}}}]</tool_call>".into(), 1, true).unwrap().unwrap();
    assert_eq!(action.arguments.mcp.unwrap().arguments["nested"][1], 4);
}

#[test]
fn credential_names_match_common_suffixes_in_any_case() {
    for name in [
        "OPENAI_API_KEY",
        "github_pat",
        "Service_Auth",
        "AWS_CREDENTIALS",
    ] {
        assert!(super::credential_name(name), "{name}");
    }
    for name in ["PATH", "HOME", "KEYBOARD_LAYOUT"] {
        assert!(!super::credential_name(name), "{name}");
    }
}

#[test]
fn short_values_are_not_treated_as_secrets() {
    let redactor = Redactor::with(Vec::new());
    #[cfg(feature = "mcp")]
    redactor
        .capture(vec!["1".into(), "true".into()])
        .expect("capture");
    assert_eq!(redactor.text("1 true"), "1 true");
}

#[cfg(feature = "mcp")]
#[test]
fn mcp_arguments_carrying_a_known_credential_are_refused() {
    let action: Action = serde_json::from_str(
        r#"{"name":"mcp_call","arguments":{"id":"server","mcp":{"tool":"send","arguments":{"body":"token sk-live-123456"}}}}"#,
    )
    .expect("action");
    assert!(
        Redactor::with(vec!["sk-live-123456".into()])
            .check_write(&action)
            .is_err()
    );
    assert!(
        Redactor::with(vec!["unrelated-secret".into()])
            .check_write(&action)
            .is_ok()
    );
}
