use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid TOML: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("cannot serialize TOML: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("unknown pack {id:?}; available: {available}")]
    MissingPack { id: String, available: String },

    #[error("dependency cycle between packs: {chain}")]
    Cycle { chain: String },

    #[error("invalid pack {id:?}: {reason}")]
    InvalidPack { id: String, reason: String },

    #[error("invalid gate {gate:?} in pack {pack:?}: {reason}")]
    InvalidGate {
        pack: String,
        gate: String,
        reason: String,
    },

    #[error("path {path:?} escapes the project root; refusing to render it")]
    PathEscape { path: String },

    #[error("refusing to overwrite {path}: {reason}")]
    Refused { path: String, reason: String },
}

pub type Result<T> = std::result::Result<T, PolicyError>;
