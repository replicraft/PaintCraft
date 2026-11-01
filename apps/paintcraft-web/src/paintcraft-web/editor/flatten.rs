//! Publication of an already rendered flatten transaction; never import a session.
use super::*;
use paintcraft_editor::session::PreparedEdit;

impl Editor {
    pub(crate) fn publish_flatten(&mut self, prepared: PreparedEdit) -> Result<()> {
        let before = self.session.ticket();
        let receipt = self.session.publish_prepared(prepared, || Ok(()))?;
        self.target = receipt.aliases.get("flattened").copied().or(self.target);
        self.retarget();
        self.record_publication(before, &receipt);
        Ok(())
    }
}
