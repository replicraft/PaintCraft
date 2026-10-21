//! Stable machine error codes (`paintcraft.error/v1`) and the CLI exit-code map.

use std::fmt;

/// Codes are part of the wire contract; never rename one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    InputSchema,
    PolicyUnknown,
    ReferenceUnknownAlias,
    ReferenceUnknownId,
    TargetInvalid,
    CapabilityUnsupported,
    ConflictStaleBase,
    ResourceLimit,
    OperationCancelled,
    CodecFailed,
    StorageFailed,
    InvariantViolation,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputSchema => "input.schema",
            Self::PolicyUnknown => "policy.unknown",
            Self::ReferenceUnknownAlias => "reference.unknown_alias",
            Self::ReferenceUnknownId => "reference.unknown_id",
            Self::TargetInvalid => "target.invalid",
            Self::CapabilityUnsupported => "capability.unsupported",
            Self::ConflictStaleBase => "conflict.stale_base",
            Self::ResourceLimit => "resource.limit",
            Self::OperationCancelled => "operation.cancelled",
            Self::CodecFailed => "codec.failed",
            Self::StorageFailed => "storage.failed",
            Self::InvariantViolation => "invariant.violation",
        }
    }

    /// Process exit status, per the CLI exit-code table.
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::InvariantViolation => 1,
            Self::InputSchema
            | Self::ReferenceUnknownAlias
            | Self::ReferenceUnknownId
            | Self::TargetInvalid => 2,
            Self::CapabilityUnsupported | Self::PolicyUnknown => 3,
            Self::ConflictStaleBase => 4,
            Self::ResourceLimit | Self::OperationCancelled => 5,
            Self::CodecFailed | Self::StorageFailed => 6,
        }
    }
}

/// A pre-publication failure. `message` is diagnostic and never part of conformance.
#[derive(Clone, Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    pub operation_index: Option<usize>,
    allocation: Option<super::resources::AllocationDenial>,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            operation_index: None,
            allocation: None,
        }
    }

    /// Internal admission context; excluded from error/v1 serialization/equality.
    pub fn allocation_denial(&self) -> Option<&super::resources::AllocationDenial> {
        if self.code == ErrorCode::ResourceLimit {
            self.allocation.as_ref()
        } else {
            None
        }
    }
    pub(super) fn denied(budget: super::resources::SharedBudget, bytes: u64) -> Self {
        let mut error = Self::new(ErrorCode::ResourceLimit, "shared byte budget is full");
        error.allocation = Some(super::resources::AllocationDenial::new(budget, bytes));
        error
    }
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }
    pub fn with_allocation_requirement(
        mut self,
        class: super::resources::AllocationClass,
        bytes: u64,
    ) -> Self {
        if let Some(denial) = &mut self.allocation {
            denial.require(class, bytes);
        }
        self
    }
    pub fn at(mut self, operation_index: usize) -> Self {
        self.operation_index = Some(operation_index);
        self
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
            && self.message == other.message
            && self.operation_index == other.operation_index
    }
}
impl Eq for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_has_a_unique_name_and_exit_status_in_range() {
        let all = [
            ErrorCode::InputSchema,
            ErrorCode::PolicyUnknown,
            ErrorCode::ReferenceUnknownAlias,
            ErrorCode::ReferenceUnknownId,
            ErrorCode::TargetInvalid,
            ErrorCode::CapabilityUnsupported,
            ErrorCode::ConflictStaleBase,
            ErrorCode::ResourceLimit,
            ErrorCode::OperationCancelled,
            ErrorCode::CodecFailed,
            ErrorCode::StorageFailed,
            ErrorCode::InvariantViolation,
        ];
        let mut names: Vec<_> = all.iter().map(|c| c.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
        assert!(all.iter().all(|c| (1..=6).contains(&c.exit_code())));
    }

    #[test]
    fn exit_codes_match_the_cli_table() {
        assert_eq!(ErrorCode::ConflictStaleBase.exit_code(), 4);
        assert_eq!(ErrorCode::PolicyUnknown.exit_code(), 3);
        assert_eq!(ErrorCode::StorageFailed.exit_code(), 6);
        assert_eq!(ErrorCode::ResourceLimit.exit_code(), 5);
    }
}
