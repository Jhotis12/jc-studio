use thiserror::Error;

/// Errors produced by the domain layer.
///
/// These are *programming* and *validation* errors, not infrastructure failures.
/// Anything that can fail because of the outside world (disk, network, subprocess)
/// belongs to the calling crate and should be wrapped there.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CoreError {
    #[error("invalid {kind} id: {value:?}")]
    InvalidId { kind: &'static str, value: String },

    #[error("illegal {entity} transition: {from} -> {to}")]
    InvalidTransition {
        entity: &'static str,
        from: &'static str,
        to: &'static str,
    },

    #[error("unknown value for {field}: {value:?}")]
    InvalidValue { field: &'static str, value: String },

    #[error("empty field: {field}")]
    EmptyField { field: &'static str },
}

impl CoreError {
    pub(crate) fn invalid_id(kind: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidId {
            kind,
            value: value.into(),
        }
    }

    pub(crate) fn unknown(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidValue {
            field,
            value: value.into(),
        }
    }
}
