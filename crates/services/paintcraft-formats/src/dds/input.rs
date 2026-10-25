use super::*;
use std::{cell::RefCell, io::Read};
pub(super) struct Control<'a> {
    pub check: &'a dyn Fn() -> Result<()>,
    failure: RefCell<Option<Error>>,
}
impl<'a> Control<'a> {
    pub fn new(check: &'a dyn Fn() -> Result<()>) -> Self {
        Self {
            check,
            failure: RefCell::new(None),
        }
    }
    pub fn io_failure(&self, error: Error) -> std::io::Error {
        *self.failure.borrow_mut() = Some(error);
        std::io::Error::other("DDS operation rejected")
    }
    pub fn error(&self, otherwise: Error) -> Error {
        self.failure.borrow_mut().take().unwrap_or(otherwise)
    }
    pub fn map(&self, error: ::dds::DecodingError) -> Error {
        let error = match error {
            ::dds::DecodingError::MemoryLimitExceeded => {
                resource("DDS decoder workspace limit exceeded")
            }
            error => codec(format!("DDS decode failed: {error}")),
        };
        self.error(error)
    }
}
pub(super) struct Input<'a, 'b> {
    pub remaining: &'a [u8],
    pub control: &'b Control<'b>,
}
impl Read for Input<'_, '_> {
    fn read(&mut self, destination: &mut [u8]) -> std::io::Result<usize> {
        if let Err(error) = (self.control.check)() {
            *self.control.failure.borrow_mut() = Some(error);
            // read_exact retries Interrupted. Use a non-retryable error, then
            // restore the original canonical cancellation at the adapter edge.
            return Err(std::io::Error::other("DDS input cancelled"));
        }
        self.remaining.read(destination)
    }
}
