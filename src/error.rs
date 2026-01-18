use std::io;
use thiserror::Error;

/// Error type for CSV sniffing operations.
#[derive(Error, Debug)]
pub enum SnifferError {
    /// IO error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// CSV parsing error.
    #[error("CSV parsing error: {0}")]
    Csv(#[from] csv::Error),

    /// No valid dialect could be detected.
    #[error("Could not detect CSV dialect: {0}")]
    NoDialectDetected(String),

    /// Empty file or no data.
    #[error("Empty file or no data to analyze")]
    EmptyData,

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Result type alias for sniffing operations.
pub type Result<T> = std::result::Result<T, SnifferError>;
