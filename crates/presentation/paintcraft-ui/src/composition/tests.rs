use super::*;
fn owner() -> Owner {
    Owner {
        session: 1,
        state: 2,
        publication: 0,
        interface: 1,
    }
}
fn frame(
    ctx: &egui::Context,
    field: &mut Field,
    text: &mut String,
    budget: &SharedBudget,
    events: Vec<Event>,
) -> bool {
    let mut refused = false;
    let mut output = ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            refused = field
                .show(
                    ui,
                    text,
                    Request {
                        host_id: Id::unique("owned-field"),
                        owner: owner(),
                        cancel: false,
                        budget,
                    },
                    |ui, text, id| egui::TextEdit::singleline(text).id(id).show(ui),
                )
                .refused;
        },
    );
    output.textures_delta.clear();
    refused
}
#[test]
fn unknown_widget_history_is_refused_without_clearing_it_and_ordinary_editing_works() {
    let ctx = egui::Context::default();
    let mut field = Field::default();
    let id = field.id(Id::unique("owned-field"));
    let mut state = TextEditState::default();
    let range =
        egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(4));
    let mut history = state.undoer();
    history.add_undo(&(range, "prior".into()));
    history.add_undo(&(range, "name".into()));
    state.set_undoer(history);
    state.cursor.set_char_range(Some(range));
    state.store(&ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
    let mut text = String::from("name");
    let budget = SharedBudget::new(1 << 20);
    assert!(frame(
        &ctx,
        &mut field,
        &mut text,
        &budget,
        vec![Event::Ime(ImeEvent::Preedit {
            text: "provisional".into(),
            active_range_chars: None,
        })]
    ));
    assert_eq!(text, "name");
    assert!(field.active.is_none());
    let mut state = TextEditState::load(&ctx, id).unwrap();
    let mut history = state.undoer();
    let mut at = (state.cursor.char_range().unwrap(), text.clone());
    let mut found = false;
    while let Some(previous) = history.undo(&at).cloned() {
        assert_ne!(previous.1, "provisional");
        found |= previous.1 == "prior";
        at = previous;
    }
    assert!(found, "unknown original history was preserved");
    state.cursor.set_char_range(Some(range));
    state.store(&ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
    assert!(!frame(
        &ctx,
        &mut field,
        &mut text,
        &budget,
        vec![Event::Text("ordinary".into())]
    ));
    assert_eq!(text, "ordinary");
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn composition_budget_wakes_can_reenter_context_on_growth_and_denial() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let run = std::thread::spawn(|| {
        let ctx = egui::Context::default();
        let mut field = Field::default();
        let mut text = String::from("name");
        let budget = SharedBudget::new(128 << 10);
        assert!(!frame(&ctx, &mut field, &mut text, &budget, Vec::new()));
        let id = field.id(Id::unique("owned-field"));
        ctx.memory_mut(|m| m.request_focus(id));
        let completed = Arc::new(AtomicUsize::new(0));
        let after_reentry = completed.clone();
        let wake_ctx = ctx.clone();
        let watch = budget
            .watch(128 << 10, move || {
                wake_ctx.request_repaint();
                after_reentry.fetch_add(1, Ordering::AcqRel);
            })
            .unwrap();
        assert!(!frame(
            &ctx,
            &mut field,
            &mut text,
            &budget,
            vec![Event::Ime(ImeEvent::Preedit {
                text: "仮".into(),
                active_range_chars: Some(0..1),
            })]
        ));
        assert!(field.active.is_some());
        assert!(completed.load(Ordering::Acquire) > 0);
        watch.observe_pressure();
        let before_denial = completed.load(Ordering::Acquire);
        assert!(frame(
            &ctx,
            &mut field,
            &mut text,
            &budget,
            vec![Event::Ime(ImeEvent::Preedit {
                text: "x".repeat(4096),
                active_range_chars: None,
            })]
        ));
        assert!(completed.load(Ordering::Acquire) > before_denial);
        assert!(field.active.is_none());
        assert_eq!(text, "name");
        assert_eq!(budget.usage().reserved, 0);
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !run.is_finished() {
        assert!(
            std::time::Instant::now() < deadline,
            "composition held a Context lock during budget wake"
        );
        std::thread::yield_now();
    }
    run.join().unwrap();
}
