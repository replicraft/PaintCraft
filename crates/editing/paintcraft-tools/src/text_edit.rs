use super::*;
impl ToolState {
    pub fn text_target(&self) -> Option<(u64, Uuid)> {
        self.text.as_ref().map(|text| (text.token, text.layer))
    }
    /// Exact end-relative replacement. Reject unsupported characters, capacity,
    /// stale targets and allocation failure before changing the draft.
    pub fn replace_text_tail(
        &mut self,
        token: u64,
        delete_chars: usize,
        typed: &str,
    ) -> paintcraft_core::types::Result<()> {
        use paintcraft_core::types::{Error, ErrorCode};
        let invalid = || {
            Error::new(
                ErrorCode::InputSchema,
                "Text exceeds the draft limit or contains unsupported characters",
            )
        };
        let draft = self
            .text
            .as_mut()
            .filter(|t| t.token == token)
            .ok_or_else(|| Error::new(ErrorCode::ConflictStaleBase, "Text draft changed"))?;
        let count = draft.text.chars().count();
        let keep = count.checked_sub(delete_chars).ok_or_else(invalid)?;
        if keep
            .checked_add(typed.chars().count())
            .is_none_or(|n| n > paintcraft_raster::font::TEXT_CHARS_MAX)
            || typed.chars().any(|c| {
                if draft.style.is_some() {
                    c != '\n' && c.is_control()
                } else {
                    !paintcraft_raster::font::supported(c)
                }
            })
        {
            return Err(invalid());
        }
        let boundary = draft
            .text
            .char_indices()
            .nth(keep)
            .map_or(draft.text.len(), |(i, _)| i);
        let mut result = String::new();
        result
            .try_reserve_exact(boundary.checked_add(typed.len()).ok_or_else(invalid)?)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Text draft allocation failed"))?;
        result.push_str(&draft.text[..boundary]);
        result.push_str(typed);
        if result != draft.text {
            draft.text = result;
            self.preview_revision.advance();
        }
        Ok(())
    }
    pub fn backspace(&mut self) {
        if let Some(t) = &mut self.text
            && t.text.pop().is_some()
        {
            self.preview_revision.advance();
        }
    }

    pub fn cancel_text(&mut self) {
        if self.text.take().is_some() {
            self.preview_revision.advance();
        }
    }

    pub fn clear_text(&mut self) {
        if let Some(text) = &mut self.text
            && !text.text.is_empty()
        {
            text.text.clear();
            self.preview_revision.advance();
        }
    }
    /// Borrow the active draft without cloning its text or resolving fonts.
    pub fn text_content(&self) -> Option<&str> {
        self.text.as_ref().map(|text| text.text.as_str())
    }
    /// Freeze the text intention while retaining the editable draft. A host can
    /// prepare/validate/publicize it before dropping the draft on success.
    pub fn pending_text_action(&self) -> Option<Action> {
        let t = self.text.as_ref()?;
        if t.text.is_empty() {
            return None;
        }
        Some(Action::Text {
            style: t.style.clone(),
            background: t.background,
            layer: t.layer,
            at: t.at,
            text: t.text.clone(),
            scale: t.scale,
            color: t.color,
        })
    }
    pub fn commit_text(&mut self) -> Option<(u64, Action)> {
        if let Some(t) = self.text.take() {
            self.preview_revision.advance();
            if t.text.is_empty() {
                return None;
            }
            return Some((
                t.token,
                Action::Text {
                    style: t.style,
                    background: t.background,
                    layer: t.layer,
                    at: t.at,
                    text: t.text,
                    scale: t.scale,
                    color: t.color,
                },
            ));
        }
        None
    }
    pub fn type_text(&mut self, typed: &str) -> usize {
        let Some(t) = &mut self.text else { return 0 };
        let mut skipped = 0;
        let previous_len = t.text.len();
        let mut count = t.text.chars().count();
        for c in typed.chars() {
            if !(if t.style.is_some() {
                c == '\n' || !c.is_control()
            } else {
                paintcraft_raster::font::supported(c)
            }) {
                skipped += 1;
            } else if count < paintcraft_raster::font::TEXT_CHARS_MAX {
                t.text.push(c);
                count += 1;
            }
        }
        if t.text.len() != previous_len {
            self.preview_revision.advance();
        }
        skipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_tail_replacement_is_scalar_bounded_and_transactional() {
        let mut tools = ToolState {
            text_style: Some(Default::default()),
            ..Default::default()
        };
        tools.begin_text(Uuid::nil(), (0, 0));
        let token = tools.text_target().unwrap().0;
        tools.replace_text_tail(token, 0, "a😀日").unwrap();
        tools.replace_text_tail(token, 2, "本").unwrap();
        assert_eq!(tools.text_content(), Some("a本"));
        let revision = tools.preview_revision();
        for (target, delete, text) in [(token + 1, 0, "x"), (token, 3, ""), (token, 0, "\0")] {
            assert!(tools.replace_text_tail(target, delete, text).is_err());
            assert_eq!(tools.text_content(), Some("a本"));
            assert_eq!(tools.preview_revision(), revision);
        }
    }
    #[test]
    fn full_capacity_result_never_silently_truncates() {
        let mut tools = ToolState {
            text_style: Some(Default::default()),
            ..Default::default()
        };
        tools.begin_text(Uuid::nil(), (0, 0));
        let token = tools.text_target().unwrap().0;
        tools
            .replace_text_tail(
                token,
                0,
                &"x".repeat(paintcraft_raster::font::TEXT_CHARS_MAX),
            )
            .unwrap();
        let revision = tools.preview_revision();
        assert!(tools.replace_text_tail(token, 0, "y").is_err());
        assert_eq!(
            tools.text_content().unwrap().len(),
            paintcraft_raster::font::TEXT_CHARS_MAX
        );
        assert_eq!(tools.preview_revision(), revision);
        tools
            .replace_text_tail(token, paintcraft_raster::font::TEXT_CHARS_MAX, "result")
            .unwrap();
        let Some((_, Action::Text { text, .. })) = tools.commit_text() else {
            panic!()
        };
        assert_eq!(text, "result");
    }
}
