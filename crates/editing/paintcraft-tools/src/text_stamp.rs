//! Exact draft acknowledgement; capture never transfers editable ownership.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};

/// Opaque, process-local identity of one exact text intention. Its revision is
/// independent of tool/picker/preview changes and prevents content ABA matches.
/// Hosts must additionally bind their session, document and publication attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextStamp {
    token: u64,
    layer: Uuid,
    revision: PreviewRevision,
}
impl TextStamp {
    pub fn draft_token(self) -> u64 {
        self.token
    }
    pub fn layer(self) -> Uuid {
        self.layer
    }
}
/// One bounded owned action, with a fresh outbox token even for an unchanged
/// draft retry. A host decides when to queue, revoke or acknowledge this attempt.
#[derive(Debug)]
pub struct TextSubmission {
    token: u64,
    stamp: TextStamp,
    action: Action,
}
impl TextSubmission {
    pub fn stamp(&self) -> TextStamp {
        self.stamp
    }
    pub fn token(&self) -> u64 {
        self.token
    }
    pub fn into_parts(self) -> (u64, TextStamp, Action) {
        (self.token, self.stamp, self.action)
    }
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn copy_string(source: &str) -> Result<String> {
    let mut result = String::new();
    result
        .try_reserve_exact(source.len())
        .map_err(|_| resource("Cannot capture editable text"))?;
    result.push_str(source);
    Ok(result)
}
fn copy_style(source: &TextOptions) -> Result<TextOptions> {
    // Same selected text renderer limits, applied before cloning the family.
    if source.family.is_empty() || source.family.len() > 256 || !(1..=512).contains(&source.points)
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Text style exceeds the renderer limits",
        ));
    }
    Ok(TextOptions {
        family: copy_string(&source.family)?,
        points: source.points,
        bold: source.bold,
        italic: source.italic,
        underline: source.underline,
        strikeout: source.strikeout,
        alignment: source.alignment,
        background_fill: source.background_fill,
        editable_until_save: source.editable_until_save,
    })
}
impl ToolState {
    /// Read only small identity fields. Public text_style/color/options are
    /// future defaults; the private TextEdit owns the actual stamped values.
    pub fn text_stamp(&self) -> Option<TextStamp> {
        self.text.as_ref().map(|text| TextStamp {
            token: text.token,
            layer: text.layer,
            revision: text.revision,
        })
    }
    pub fn text_stamp_current(&self, stamp: TextStamp) -> bool {
        self.text_stamp() == Some(stamp)
    }
    /// Called only after a host's successful matching publication. No stale
    /// acknowledgement may remove a changed, canceled or replacement draft.
    /// An explicitly accepted empty draft can also be acknowledged locally.
    pub fn acknowledge_text(&mut self, stamp: TextStamp) -> bool {
        if !self.text_stamp_current(stamp) {
            return false;
        }
        self.text = None;
        self.preview_revision.advance();
        true
    }
    /// Capture once per actual submission, never once per layout frame. Failed
    /// capture leaves both the draft and token counter unchanged. Empty drafts
    /// produce no action and remain available for explicit acknowledgement.
    pub fn capture_text_submission(&mut self) -> Result<Option<TextSubmission>> {
        let Some(text) = self.text.as_ref().filter(|text| !text.text.is_empty()) else {
            return Ok(None);
        };
        let maximum = paintcraft_raster::font::TEXT_CHARS_MAX;
        if text.text.len() > maximum * 4 || text.text.chars().count() > maximum {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Text exceeds the draft limit",
            ));
        }
        let token = self
            .next_gesture
            .checked_add(1)
            .ok_or_else(|| resource("Text submission tokens exhausted"))?;
        let stamp = self.text_stamp().expect("captured text draft");
        let action = Action::Text {
            style: text.style.as_ref().map(copy_style).transpose()?,
            background: text.background,
            layer: text.layer,
            at: text.at,
            text: copy_string(&text.text)?,
            scale: text.scale,
            color: text.color,
        };
        self.next_gesture = token;
        Ok(Some(TextSubmission {
            token,
            stamp,
            action,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_submission_token_preserves_editable_intention() {
        let mut tools = ToolState::default();
        tools.begin_text(Uuid::nil(), (0, 0));
        tools.type_text("Keep");
        tools.next_gesture = u64::MAX;
        let stamp = tools.text_stamp().unwrap();
        let preview = tools.preview_revision();
        assert_eq!(
            tools.capture_text_submission().unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(tools.next_gesture, u64::MAX);
        assert!(tools.text_stamp_current(stamp));
        assert_eq!(tools.preview_revision(), preview);
        assert_eq!(tools.text_content(), Some("Keep"));
    }
}
