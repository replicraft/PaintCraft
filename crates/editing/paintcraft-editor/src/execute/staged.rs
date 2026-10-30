//! Final private result assembly; publication remains on the Session owner.
use super::*;
pub(super) fn finish(stage: Stage<'_>) -> Staged {
    let aliases = stage
        .aliases
        .into_iter()
        .filter_map(|(alias, id)| {
            id.filter(|id| stage.doc.layers.contains_key(id))
                .map(|id| (alias, id))
        })
        .collect();
    let filter_aliases = filters::retained_aliases(&stage.doc, stage.filter_aliases);
    Staged {
        doc: stage.doc,
        selection: stage.selection,
        aliases,
        filter_aliases,
    }
}
