//! Metadata-only ordered filter edits: original rasters and versions stay intact.
use super::*;
use paintcraft_commands::{FilterOp, FilterRef};
use paintcraft_document::{FilterInstance, FilterStack, filters::FILTERS_MAX};

pub(super) type Aliases = HashMap<String, Option<(Uuid, Uuid)>>;
pub type PublishedAliases = BTreeMap<String, (Uuid, Uuid)>;

impl Stage<'_> {
    fn filter_position(&self, layer: Uuid, reference: &FilterRef) -> Result<usize> {
        let id = match reference {
            FilterRef::Id(id) => *id,
            FilterRef::Alias(alias) => {
                let (owner, id) =
                    self.filter_aliases
                        .get(alias)
                        .and_then(|v| *v)
                        .ok_or_else(|| {
                            Error::new(ErrorCode::ReferenceUnknownAlias, "Unknown filter alias")
                        })?;
                if owner != layer {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Filter alias belongs to another layer",
                    ));
                }
                id
            }
        };
        self.doc.layers[&layer]
            .filters
            .iter()
            .position(|f| f.id() == id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ReferenceUnknownId,
                    "Unknown filter ID on this layer",
                )
            })
    }
    fn filter_alias_available(&self, alias: &str) -> Result<()> {
        if self.filter_aliases.contains_key(alias) {
            Err(Error::new(ErrorCode::InputSchema, "Duplicate filter alias"))
        } else {
            Ok(())
        }
    }
    fn filter_id(&self, layer: Uuid, operation: usize) -> Result<Uuid> {
        let id = deterministic_id(&[
            "filter",
            &layer.to_string(),
            &self.base_state,
            self.digest,
            &operation.to_string(),
        ]);
        if self.doc.layers[&layer].filters.iter().any(|f| f.id() == id) {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "Deterministic filter ID collision",
            ));
        }
        Ok(id)
    }
    fn filter_instances(&self, layer: Uuid, grow: bool) -> Result<Vec<FilterInstance>> {
        let stack = &self.doc.layers[&layer].filters;
        let size = stack.len() + usize::from(grow);
        if size > FILTERS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Layer filter count limit",
            ));
        }
        let mut instances = Vec::new();
        instances
            .try_reserve_exact(size)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Cannot stage filter stack"))?;
        instances.extend(stack.iter().cloned());
        Ok(instances)
    }
    pub(super) fn edit_filter(&mut self, index: usize, op: &FilterOp) -> Result<()> {
        (self.check)()?;
        let layer = match op {
            FilterOp::Add { layer, .. }
            | FilterOp::Update { layer, .. }
            | FilterOp::SetEnabled { layer, .. }
            | FilterOp::Reorder { layer, .. }
            | FilterOp::Remove { layer, .. }
            | FilterOp::Duplicate { layer, .. } => self.resolve(layer)?,
        };
        let mut instances;
        match op {
            FilterOp::Add {
                alias,
                descriptor,
                enabled,
                to_index,
                ..
            } => {
                self.filter_alias_available(alias)?;
                let position = to_index.unwrap_or(self.doc.layers[&layer].filters.len());
                if position > self.doc.layers[&layer].filters.len() {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Filter insertion index outside stack",
                    ));
                }
                let id = self.filter_id(layer, index)?;
                instances = self.filter_instances(layer, true)?;
                instances.insert(position, FilterInstance::new(id, *enabled, *descriptor));
                self.filter_aliases.insert(alias.clone(), Some((layer, id)));
            }
            FilterOp::Duplicate {
                filter,
                alias,
                to_index,
                ..
            } => {
                self.filter_alias_available(alias)?;
                let source = self.filter_position(layer, filter)?;
                let position = to_index.unwrap_or(source + 1);
                if position > self.doc.layers[&layer].filters.len() {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Filter insertion index outside stack",
                    ));
                }
                let original = &self.doc.layers[&layer].filters.as_slice()[source];
                let id = self.filter_id(layer, index)?;
                let copy = FilterInstance::new(id, original.enabled(), original.descriptor());
                instances = self.filter_instances(layer, true)?;
                instances.insert(position, copy);
                self.filter_aliases.insert(alias.clone(), Some((layer, id)));
            }
            FilterOp::Update {
                filter, descriptor, ..
            } => {
                let position = self.filter_position(layer, filter)?;
                let original = &self.doc.layers[&layer].filters.as_slice()[position];
                if original.descriptor() == *descriptor {
                    return Ok(());
                }
                let replacement =
                    FilterInstance::new(original.id(), original.enabled(), *descriptor);
                instances = self.filter_instances(layer, false)?;
                instances[position] = replacement;
            }
            FilterOp::SetEnabled {
                filter, enabled, ..
            } => {
                let position = self.filter_position(layer, filter)?;
                let original = &self.doc.layers[&layer].filters.as_slice()[position];
                if original.enabled() == *enabled {
                    return Ok(());
                }
                let replacement =
                    FilterInstance::new(original.id(), *enabled, original.descriptor());
                instances = self.filter_instances(layer, false)?;
                instances[position] = replacement;
            }
            FilterOp::Reorder {
                filter, to_index, ..
            } => {
                let position = self.filter_position(layer, filter)?;
                if *to_index >= self.doc.layers[&layer].filters.len() {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Filter reorder index outside stack",
                    ));
                }
                if position == *to_index {
                    return Ok(());
                }
                instances = self.filter_instances(layer, false)?;
                let moved = instances.remove(position);
                instances.insert(*to_index, moved);
            }
            FilterOp::Remove { filter, .. } => {
                let position = self.filter_position(layer, filter)?;
                instances = self.filter_instances(layer, false)?;
                let removed = instances.remove(position).id();
                for alias in self
                    .filter_aliases
                    .values_mut()
                    .filter(|v| **v == Some((layer, removed)))
                {
                    *alias = None;
                }
            }
        }
        (self.check)()?;
        self.doc
            .layers
            .get_mut(&layer)
            .expect("resolved layer")
            .filters = FilterStack::new(instances)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

pub(super) fn retained_aliases(document: &Document, aliases: Aliases) -> PublishedAliases {
    aliases
        .into_iter()
        .filter_map(|(alias, qualified)| {
            qualified
                .filter(|(layer, filter)| {
                    document.layers.get(layer).is_some_and(|layer| {
                        layer.filters.iter().any(|entry| entry.id() == *filter)
                    })
                })
                .map(|qualified| (alias, qualified))
        })
        .collect()
}
