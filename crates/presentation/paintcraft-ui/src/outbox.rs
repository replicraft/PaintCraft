//! An action outbox with activation tokens.
//!
//! An immediate-mode UI may run its layout more than once per frame. Widgets
//! therefore never execute commands while drawing: they push an action tagged
//! with a stable token, and the application drains the outbox once after the
//! final pass. A token that has already been pending or executed is ignored,
//! so one user activation can never run twice, while two distinct activations
//! (distinct tokens) both run. This is in-process deduplication only.

use std::collections::{HashSet, VecDeque};

/// Identifies one user activation (for example a gesture generation or a click serial).
pub type Token = u64;

const REMEMBERED: usize = 4096;

#[derive(Debug)]
pub struct Outbox<A> {
    pending: Vec<(Token, A)>,
    seen: HashSet<Token>,
    order: VecDeque<Token>,
}

impl<A> Default for Outbox<A> {
    fn default() -> Self {
        Outbox {
            pending: Vec::new(),
            seen: HashSet::new(),
            order: VecDeque::new(),
        }
    }
}

impl<A> Outbox<A> {
    /// Queue an action. Returns `false` when the token was already queued or run.
    pub fn push(&mut self, token: Token, action: A) -> bool {
        if !self.seen.insert(token) {
            return false;
        }
        self.order.push_back(token);
        if self.order.len() > REMEMBERED
            && let Some(old) = self.order.pop_front()
        {
            self.seen.remove(&old);
        }
        self.pending.push((token, action));
        true
    }

    /// Take everything queued, in arrival order.
    pub fn drain(&mut self) -> Vec<A> {
        self.pending.drain(..).map(|(_, a)| a).collect()
    }

    /// Whether this activation is still in the deduplication window.
    pub fn has_seen(&self, token: Token) -> bool {
        self.seen.contains(&token)
    }

    /// Inspect accepted activations in arrival order without consuming them.
    pub fn pending(&self) -> impl ExactSizeIterator<Item = &A> {
        self.pending.iter().map(|(_, action)| action)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_passes_with_one_token_execute_once() {
        let mut o = Outbox::default();
        assert!(o.push(1, "add layer"));
        assert!(!o.push(1, "add layer"), "second layout pass");
        assert_eq!(o.drain(), ["add layer"]);
        assert!(!o.push(1, "add layer"), "even after the first was executed");
        assert!(o.drain().is_empty());
    }

    #[test]
    fn two_distinct_activations_both_run_in_order() {
        let mut o = Outbox::default();
        o.push(1, 'a');
        o.push(2, 'b');
        assert_eq!(o.drain(), ['a', 'b']);
        assert!(o.is_empty());
    }

    #[test]
    fn memory_is_bounded() {
        let mut o = Outbox::default();
        for t in 0..(REMEMBERED as u64 + 10) {
            o.push(t, ());
        }
        assert!(o.seen.len() <= REMEMBERED && o.order.len() <= REMEMBERED);
        o.drain();
        assert!(o.push(0, ()), "tokens older than the window may be reused");
    }
}
