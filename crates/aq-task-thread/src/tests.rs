use super::*;

fn thread(n: usize, body: &str) -> Thread {
    Thread {
        comments: (0..n)
            .map(|i| Comment {
                at: format!("2026-11-06T10:00:{i:02}Z"),
                author: "owner".into(),
                body: body.into(),
            })
            .collect(),
        more_comments: false,
        questions: vec![Question {
            decision_id: Uuid::from_u128(1),
            state: "answered".into(),
            question: "Should the notes include the deprecated API?".into(),
            answer: Some("No, leave it out.".into()),
        }],
        messages: vec![Message {
            at: "2026-11-06T11:00:00Z".into(),
            sender: "owner".into(),
            body: "AQ answer to your question".into(),
        }],
        commands: vec![
            "whoami".into(),
            "comment.list".into(),
            "task.comment.add".into(),
        ],
    }
}

#[test]
fn renders_every_section_with_the_policy_label_and_commands() {
    let text = render(&thread(2, "Found the checklist."), BUDGET_BYTES).unwrap();
    assert!(text.starts_with(HEADER));
    for part in [
        "### Comments",
        "Found the checklist.",
        "decision 00000000-0000-0000-0000-000000000001 (answered)",
        "Answer: No, leave it out.",
        "### Unread task mail",
        "`aq question show <decision_id>`",
        "whoami, comment.list, task.comment.add",
    ] {
        assert!(text.contains(part), "{part}\n{text}");
    }
    assert!(!text.contains("left out"));
}

#[test]
fn stays_within_budget_and_says_when_it_truncates() {
    let long = "x".repeat(5000);
    let text = render(&thread(40, &long), BUDGET_BYTES).unwrap();
    assert!(text.len() <= BUDGET_BYTES, "{}", text.len());
    assert!(text.contains("left out to fit the startup budget"));
    assert!(
        text.contains("x..."),
        "a long body is clipped, not dropped whole"
    );
    assert!(
        text.ends_with("whoami, comment.list, task.comment.add"),
        "the command list survives"
    );
    for budget in [200, 600, 2048] {
        assert!(render(&thread(10, &long), budget).unwrap().len() <= budget);
    }
}

#[test]
fn clip_respects_character_boundaries() {
    let (s, cut) = clip(&"é".repeat(600), 101);
    assert!(cut && s.len() <= 101 && s.ends_with("..."));
    assert_eq!(clip("  short  ", 10), ("short".to_string(), false));
    assert!(render(&Thread::default(), BUDGET_BYTES).is_none());
}
