use paintcraft_core::types::{ErrorCode, Rgba, text::TextOptions};
use paintcraft_tools::{Action, Tool, ToolState};
use uuid::Uuid;

fn draft() -> ToolState {
    let mut tools = ToolState::default();
    tools.begin_text(Uuid::from_u128(19), (3, 4));
    tools.type_text("Ink");
    tools
}
#[test]
fn captures_keep_exact_draft_and_retry_with_distinct_activation_tokens() {
    let mut tools = draft();
    let revision = tools.preview_revision();
    let stamp = tools.text_stamp().unwrap();
    let first = tools.capture_text_submission().unwrap().unwrap();
    let second = tools.capture_text_submission().unwrap().unwrap();
    assert_eq!(first.stamp(), stamp);
    assert_eq!(second.stamp(), stamp);
    assert_ne!(first.token(), second.token());
    assert!(first.token() > stamp.draft_token());
    assert_eq!(stamp.layer(), Uuid::from_u128(19));
    assert_eq!(tools.preview_revision(), revision);
    assert_eq!(tools.text_content(), Some("Ink"));
    let (_, captured, action) = first.into_parts();
    assert_eq!(captured, stamp);
    assert_eq!(Some(action), tools.pending_text_action());
    assert!(tools.acknowledge_text(stamp));
    assert!(!tools.acknowledge_text(second.stamp()));
    assert!(tools.text_content().is_none());
}
#[test]
fn every_content_mutation_invalidates_stamp_even_when_text_returns_to_same_value() {
    let mut tools = draft();
    let initial = tools.text_stamp().unwrap();
    tools.type_text("!");
    let appended = tools.text_stamp().unwrap();
    assert_ne!(initial, appended);
    tools.backspace();
    assert_eq!(tools.text_content(), Some("Ink"));
    assert!(!tools.text_stamp_current(initial));
    assert!(!tools.text_stamp_current(appended));
    let token = tools.text_target().unwrap().0;
    let before = tools.text_stamp().unwrap();
    tools.replace_text_tail(token, 0, "").unwrap();
    assert_eq!(tools.text_stamp(), Some(before));
    tools.replace_text_tail(token, 1, "t").unwrap();
    assert!(!tools.acknowledge_text(before));
    let before = tools.text_stamp().unwrap();
    tools.clear_text();
    assert!(!tools.text_stamp_current(before));
    let empty = tools.text_stamp().unwrap();
    tools.clear_text();
    tools.backspace();
    tools.type_text("");
    assert_eq!(tools.text_stamp(), Some(empty));
    assert!(tools.capture_text_submission().unwrap().is_none());
    assert!(tools.acknowledge_text(empty));
}
#[test]
fn tool_and_future_defaults_do_not_change_captured_intention_but_guarded_style_does() {
    let mut tools = draft();
    let stamp = tools.text_stamp().unwrap();
    let original = tools.pending_text_action();
    tools.choose_tool(Tool::Pan);
    tools.color = Rgba([1, 2, 3, 4]);
    tools.secondary = Rgba([5, 6, 7, 8]);
    tools.options.text_scale = 8;
    tools.text_style = Some(TextOptions {
        points: 20,
        ..Default::default()
    });
    assert!(tools.text_stamp_current(stamp));
    assert_eq!(tools.pending_text_action(), original);
    let (_, captured, action) = tools
        .capture_text_submission()
        .unwrap()
        .unwrap()
        .into_parts();
    assert_eq!(captured, stamp);
    assert_eq!(Some(action), original);
    tools.set_text_style(tools.text_style.clone().unwrap());
    let changed = tools.text_stamp().unwrap();
    assert_ne!(changed, stamp);
    assert!(!tools.acknowledge_text(stamp));
    tools.set_text_style(tools.text_style.clone().unwrap());
    assert_eq!(tools.text_stamp(), Some(changed));
    tools.secondary = Rgba([9, 10, 11, 12]);
    tools.set_text_style(tools.text_style.clone().unwrap());
    assert!(!tools.text_stamp_current(changed));
    assert!(matches!(
        tools.pending_text_action(),
        Some(Action::Text { scale: 2, .. })
    ));
}
#[test]
fn cancel_replacement_and_empty_draft_never_accept_an_older_acknowledgement() {
    let mut tools = draft();
    let old = tools.text_stamp().unwrap();
    tools.cancel_text();
    assert!(!tools.acknowledge_text(old));
    tools.begin_text(old.layer(), (3, 4));
    tools.type_text("Ink");
    assert_ne!(tools.text_stamp(), Some(old));
    assert!(!tools.acknowledge_text(old));
    let current = tools.text_stamp().unwrap();
    tools.begin_text(old.layer(), (3, 4));
    assert!(!tools.acknowledge_text(current));
    let empty = tools.text_stamp().unwrap();
    assert!(tools.capture_text_submission().unwrap().is_none());
    assert!(tools.text_stamp_current(empty));
    assert!(tools.acknowledge_text(empty));
}
#[test]
fn invalid_bounded_capture_preserves_draft_and_does_not_consume_a_submission_token() {
    let mut tools = draft();
    let token = tools.capture_text_submission().unwrap().unwrap().token();
    for style in [
        TextOptions {
            family: "x".repeat(257),
            ..Default::default()
        },
        TextOptions {
            family: String::new(),
            ..Default::default()
        },
        TextOptions {
            points: 513,
            ..Default::default()
        },
    ] {
        tools.set_text_style(style);
        let stamp = tools.text_stamp().unwrap();
        assert_eq!(
            tools.capture_text_submission().unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert!(tools.text_stamp_current(stamp));
        assert_eq!(tools.text_content(), Some("Ink"));
    }
    tools.set_text_style(TextOptions::default());
    assert_eq!(
        tools.capture_text_submission().unwrap().unwrap().token(),
        token + 1
    );
}
#[test]
fn legacy_destructive_commit_keeps_its_original_token_and_action_contract() {
    let mut tools = draft();
    let stamp = tools.text_stamp().unwrap();
    let expected = tools.pending_text_action().unwrap();
    tools.capture_text_submission().unwrap().unwrap();
    assert_eq!(tools.commit_text(), Some((stamp.draft_token(), expected)));
    assert!(tools.text_stamp().is_none());
    tools.begin_text(Uuid::nil(), (0, 0));
    assert!(tools.commit_text().is_none());
    assert!(tools.text_stamp().is_none());
}
