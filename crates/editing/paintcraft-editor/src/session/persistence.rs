//! Host-supplied admission for newly authored persistent document revisions.
use super::*;
use std::cell::RefCell;

/// A pure, cancellable host persistence check. The editor has no format dependency.
/// Implementations must not publish, mutate external state or acknowledge a save.
/// The document already carries its final proposed state ID. Returning an error
/// prevents owner publication, including its selection and history changes.
pub type PersistentValidator = fn(&Document, &dyn Fn() -> Result<()>) -> Result<()>;

impl Session {
    /// Configure admission for future content edits; this does not validate the
    /// opened document or rewrite retained history. All older preparations become
    /// stale, even when installing the same function again or removing the check.
    /// No-op and selection-only edits and history navigation remain available.
    pub fn set_persistent_validator(&mut self, validator: Option<PersistentValidator>) {
        self.persistent_validator = validator;
        self.preparation_policy = Arc::new(());
    }

    /// Configure a session before exposing it to its owner. Hosts must separately
    /// admit their initial document and select a check matching their save profile.
    pub fn with_persistent_validator(mut self, validator: Option<PersistentValidator>) -> Self {
        self.set_persistent_validator(validator);
        self
    }

    pub(super) fn validate_persistent(
        &self,
        document: &Document,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if let Some(validator) = self.persistent_validator {
            check()?;
            // A validator may wrap an API that catches callback errors. Preserve
            // the first caller failure even if that API later reports success.
            let failure = RefCell::new(None::<Error>);
            let checked = || {
                if let Some(error) = failure.borrow().as_ref() {
                    return Err(error.clone());
                }
                let result = check();
                if let Err(error) = &result {
                    *failure.borrow_mut() = Some(error.clone());
                }
                result
            };
            let result = validator(document, &checked);
            if let Some(error) = failure.into_inner() {
                return Err(error);
            }
            result?;
            check()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
