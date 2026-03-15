//! Typed error types for configuration loading and validation.

use std::path::PathBuf;

/// Typed configuration error for `mcp-hub`.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    /// Failed to read a configuration file from disk.
    #[error("failed to read config file '{path}': {source}")]
    Read {
        /// Path that failed to load.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
    /// Failed to parse a TOML configuration file.
    #[error("failed to parse config file '{path}': {source}")]
    Parse {
        /// Path that failed to parse.
        path: PathBuf,
        /// Underlying TOML parse error.
        source: toml::de::Error,
    },
    /// The configuration was syntactically valid but semantically invalid.
    #[error("invalid config: {0}")]
    Validation(String),
}
