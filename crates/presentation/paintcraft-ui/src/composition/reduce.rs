//! Only IME replacement is mirrored; all ordinary editing remains egui's.
use egui::{
    ImeEvent, TextBuffer,
    text::{CCursor, CCursorRange},
};

pub(super) struct Reduced {
    pub text: String,
    pub cursor: CCursorRange,
    pub composing: bool,
    pub result: Option<(String, CCursorRange)>,
}
impl Reduced {
    pub fn apply(&mut self, event: &ImeEvent) {
        match event {
            #[expect(deprecated)]
            ImeEvent::Enabled | ImeEvent::Disabled => {}
            ImeEvent::Preedit { text, .. } | ImeEvent::Commit(text)
                if (text.is_empty() && !self.composing) || text == "\n" || text == "\r" => {}
            ImeEvent::Preedit { text, .. } => {
                let mut at = self.text.delete_selected(&self.cursor);
                let start = at;
                self.text.insert_text_at(&mut at, text, usize::MAX);
                self.cursor = CCursorRange::two(start, at);
                self.composing = !text.is_empty();
            }
            ImeEvent::Commit(text) => {
                let mut at = self.text.delete_selected(&self.cursor);
                self.text.insert_text_at(&mut at, text, usize::MAX);
                self.cursor = CCursorRange::one(at);
                self.composing = false;
                self.result = Some((self.text.clone(), self.cursor));
            }
            ImeEvent::DeleteSurrounding {
                before_chars,
                after_chars,
            } => {
                self.cursor =
                    self.text
                        .delete_surrounding_chars(self.cursor, *before_chars, *after_chars);
            }
        }
    }
    pub fn at_end(text: String) -> Self {
        let cursor = CCursorRange::one(CCursor::new(text.chars().count()));
        Self {
            text,
            cursor,
            composing: false,
            result: None,
        }
    }
}
