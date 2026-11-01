//! Whole-canvas raw Source6 projection into the existing atomic flatten edit.
//! Main ingestion/hash/history are synchronous and need a separate latency gate.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_editor::session::Preparation;

const PACKED_LIMIT: u64 = crate::editor::MAX_PIXELS * 4;
pub(super) struct Transaction {
    preparation: Preparation,
    source: Arc<Document>,
    pub(super) account: Option<BudgetAccount>,
}
/// Field order matters: packed bytes are dropped before their Main lease.
pub(super) struct Pixels {
    pub(super) bytes: Vec<u8>,
    pub(super) account: BudgetAccount,
}
impl WebApp {
    pub(crate) fn noise_flatten_pending(&self) -> bool {
        self.filter_display
            .copy_action
            .as_ref()
            .is_some_and(|job| matches!(job.purpose, Some(Purpose::Flatten)))
    }
    pub(crate) fn queue_noise_flatten(&mut self, action: &paintcraft_tools::Action) -> bool {
        if !matches!(action, paintcraft_tools::Action::FlattenImage)
            || !render::stored_background(self.editor.session.document())
        {
            return false;
        }
        if let Err(error) = self.capture_noise_flatten() {
            self.message = format!("Edit failed: {error}");
        }
        true
    }
    fn capture_noise_flatten(&mut self) -> Result<()> {
        if self.filter_display.copy_action.is_some() || self.filter_display.picker_wanted.is_some()
        {
            return Err(rejected("Another background image action is pending"));
        }
        if self.filter_editing.is_some()
            || self.paste.is_placing()
            || self.text.is_composing()
            || self.editor.tools.is_editing_text()
            || self.editor.tools.floating_selection().is_some()
            || self.editor.tools.is_constructing_shape()
        {
            return Err(rejected("Finish the private draft before flattening"));
        }
        // The ordinary menu already called finish_text. Preserve that existing
        // placement transaction; flatten cancellation does not undo placed text.
        self.editor.tools.cancel_gesture();
        self.forget_noise_preview();
        let preparation = self.editor.session.preparation()?;
        let capture = self.filter_capture();
        // Keep the original caller through later display suspension/close ACK.
        let source = channel::SourceKey::new(capture.clone(), None, None)?;
        let mut account = self.clipboard.budget.account(AllocationClass::SharedBudget);
        // Reserve the complete raw output bound BEFORE Client::run allocates.
        // This uses the existing Main clipboard realm, not the Worker lease.
        account.resize(PACKED_LIMIT)?;
        let transaction = Transaction {
            preparation,
            source: capture.document.clone(),
            account: Some(account),
        };
        self.filter_display.copy_action = Some(Job {
            frozen: Frozen::capture(self),
            region: capture.document.canvas_rect(),
            flatten: Some(transaction),
            capture,
            source,
            spec: Value::Null,
            mask: Some(self.empty_copy_mask()),
            purpose: Some(Purpose::Flatten),
            outcome: None,
            cancelled: Rc::new(Cell::new(false)),
            #[cfg(target_arch = "wasm32")]
            control: Rc::default(),
        });
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) async fn render(
    client: &channel::Client,
    capture: &Capture,
    region: Rect,
    account: BudgetAccount,
) -> Result<Pixels> {
    let reply = client
        .run(
            Request {
                revision: Revision {
                    draft: capture.draft,
                    generation: 1,
                },
                edit: None,
                render: true,
                view: View::new(&capture.document, region, 0)?,
            },
            1,
        )
        .await?;
    let bytes = reply.pixels.ok_or_else(|| rejected("No flatten pixels"))?;
    // Full reservation remains owned even on a rejected capacity. Client::run
    // already checks exact length, full BASE bindings, level and canvas ROI.
    if bytes.capacity() as u64 > PACKED_LIMIT {
        return Err(rejected("Flatten raw capacity exceeds its admission"));
    }
    Ok(Pixels { bytes, account })
}
pub(super) fn publish(app: &mut WebApp, transaction: Transaction, pixels: Pixels) -> Result<()> {
    if pixels.bytes.capacity() as u64 > pixels.account.bytes() {
        return Err(rejected("Flatten pixels exceed their retained Main lease"));
    }
    let Transaction {
        preparation,
        source,
        account,
    } = transaction;
    drop(account);
    let Pixels {
        bytes,
        account: lease,
    } = pixels;
    let result = preparation
        .flatten(
            move |doc, check| {
                check()?;
                if !std::ptr::eq(doc, source.as_ref()) {
                    return Err(Error::new(
                        ErrorCode::ConflictStaleBase,
                        "Flatten source changed",
                    ));
                }
                Ok(bytes)
            },
            paintcraft_raster::Admission::new(crate::editor::MAX_PIXELS * 4),
            || Ok(()),
        )
        .and_then(|prepared| app.editor.publish_flatten(prepared));
    // flatten consumes/drops its packed Vec before returning, including errors.
    drop(lease);
    result
}
