//! Metadata-only ordered style edits: original rasters and versions stay intact.
use super::*;
use paintcraft_commands::{StyleOp, StyleRef};
use paintcraft_document::{StyleInstance, StyleStack, styles::STYLES_MAX};

pub(super) type Aliases = HashMap<String, Option<(Uuid, Uuid)>>;
pub type PublishedAliases = BTreeMap<String, (Uuid, Uuid)>;

impl Stage<'_> {
    pub(super) fn style_position(&self, layer: Uuid, reference: &StyleRef) -> Result<usize> {
        let id = match reference {
            StyleRef::Id(id) => *id,
            StyleRef::Alias(alias) => {
                let (owner, id) =
                    self.style_aliases
                        .get(alias)
                        .and_then(|v| *v)
                        .ok_or_else(|| {
                            Error::new(ErrorCode::ReferenceUnknownAlias, "Unknown style alias")
                        })?;
                if owner != layer {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Style alias belongs to another layer",
                    ));
                }
                id
            }
        };
        self.doc.layers[&layer]
            .styles
            .iter()
            .position(|f| f.id() == id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ReferenceUnknownId,
                    "Unknown style ID on this layer",
                )
            })
    }
    fn style_alias_available(&self, alias: &str) -> Result<()> {
        if self.style_aliases.contains_key(alias) {
            Err(Error::new(ErrorCode::InputSchema, "Duplicate style alias"))
        } else {
            Ok(())
        }
    }
    fn style_id(&self, layer: Uuid, operation: usize) -> Result<Uuid> {
        let id = deterministic_id(&[
            "layer-style",
            &layer.to_string(),
            &self.base_state,
            self.digest,
            &operation.to_string(),
        ]);
        if self.doc.layers[&layer].styles.iter().any(|f| f.id() == id) {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "Deterministic style ID collision",
            ));
        }
        Ok(id)
    }
    pub(super) fn style_instances(&self, layer: Uuid, grow: bool) -> Result<Vec<StyleInstance>> {
        let stack = &self.doc.layers[&layer].styles;
        let size = stack.len() + usize::from(grow);
        if size > STYLES_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Layer style count limit",
            ));
        }
        let mut instances = Vec::new();
        instances
            .try_reserve_exact(size)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Cannot stage style stack"))?;
        instances.extend(stack.iter().copied());
        Ok(instances)
    }
    pub(super) fn edit_style(&mut self, index: usize, op: &StyleOp) -> Result<()> {
        (self.check)()?;
        let layer = self.resolve(op.references().0)?;
        let mut instances;
        match op {
            StyleOp::Add {
                alias,
                descriptor,
                enabled,
                to_index,
                ..
            } => {
                self.style_alias_available(alias)?;
                let position = to_index.unwrap_or(self.doc.layers[&layer].styles.len());
                if position > self.doc.layers[&layer].styles.len() {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Style insertion index outside stack",
                    ));
                }
                let id = self.style_id(layer, index)?;
                instances = self.style_instances(layer, true)?;
                instances.insert(position, StyleInstance::new(id, *enabled, *descriptor)?);
                self.style_aliases.insert(alias.clone(), Some((layer, id)));
            }
            StyleOp::Duplicate {
                style,
                alias,
                to_index,
                ..
            } => {
                self.style_alias_available(alias)?;
                let source = self.style_position(layer, style)?;
                let position = to_index.unwrap_or(source + 1);
                if position > self.doc.layers[&layer].styles.len() {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Style insertion index outside stack",
                    ));
                }
                let original = &self.doc.layers[&layer].styles.as_slice()[source];
                let id = self.style_id(layer, index)?;
                let copy = original.with_id(id)?;
                instances = self.style_instances(layer, true)?;
                instances.insert(position, copy);
                self.style_aliases.insert(alias.clone(), Some((layer, id)));
            }
            StyleOp::Update {
                style, descriptor, ..
            } => {
                let position = self.style_position(layer, style)?;
                let original = &self.doc.layers[&layer].styles.as_slice()[position];
                if original.descriptor() == *descriptor {
                    return Ok(());
                }
                let replacement = original.with_descriptor(*descriptor);
                instances = self.style_instances(layer, false)?;
                instances[position] = replacement;
            }
            StyleOp::SetEnabled { style, enabled, .. } => {
                let position = self.style_position(layer, style)?;
                let original = &self.doc.layers[&layer].styles.as_slice()[position];
                if original.enabled() == *enabled {
                    return Ok(());
                }
                let replacement = original.with_enabled(*enabled);
                instances = self.style_instances(layer, false)?;
                instances[position] = replacement;
            }
            StyleOp::Move {
                style, to_index, ..
            } => {
                let position = self.style_position(layer, style)?;
                if *to_index >= self.doc.layers[&layer].styles.len() {
                    return Err(Error::new(
                        ErrorCode::TargetInvalid,
                        "Style move index outside stack",
                    ));
                }
                if position == *to_index {
                    return Ok(());
                }
                instances = self.style_instances(layer, false)?;
                let moved = instances.remove(position);
                instances.insert(*to_index, moved);
            }
            StyleOp::Remove { style, .. } => {
                let position = self.style_position(layer, style)?;
                instances = self.style_instances(layer, false)?;
                let removed = instances.remove(position).id();
                for alias in self
                    .style_aliases
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
            .styles = StyleStack::new_checked(instances, self.check)?;
        Ok(())
    }
}

pub(super) fn retained_aliases(document: &Document, aliases: Aliases) -> PublishedAliases {
    aliases
        .into_iter()
        .filter_map(|(alias, qualified)| {
            qualified
                .filter(|(layer, style)| {
                    document
                        .layers
                        .get(layer)
                        .is_some_and(|layer| layer.styles.iter().any(|entry| entry.id() == *style))
                })
                .map(|qualified| (alias, qualified))
        })
        .collect()
}
