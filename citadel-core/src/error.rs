use thiserror::Error;

/// The single error type for all citadel-core operations.
///
/// Replaces the scattered `StorageError`, bare `String` errors, and `()` error
/// types used in `FromStr` implementations. Every consumer in the codebase
/// receives structured, matchable error variants instead of opaque strings.
#[derive(Debug, Error)]
pub enum CitadelError {
    #[error("[ParseError] {0}")]
    ParseError(String),

    #[error("[UnsupportedSyntax] {0}")]
    UnsupportedSyntax(String),

    #[error("[FatalPanic] {0}")]
    FatalPanic(String),

    #[error("[StackOverflow] maximum recursion depth exceeded in {location}")]
    StackOverflow { location: String },

    #[error("[InvalidSpan] {node}: line {line}, column {column} — {reason}")]
    InvalidSpan {
        node: String,
        line: u32,
        column: u32,
        reason: String,
    },

    #[error("[TreeSitterError] {0}")]
    TreeSitterError(String),

    #[error("[IoError] {0}")]
    IoError(#[from] std::io::Error),

    #[error("[DatabaseError] {0}")]
    DatabaseError(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("[DatabaseError] {0}")]
    Database(String),

    #[error("[MigrationError] {0}")]
    Migration(String),

    #[error("[MigrationError] {0}")]
    MigrationError(String),

    #[error("[NotInitialized] {0}")]
    NotInitialized(String),

    #[error("[NotFound] {0}")]
    NotFound(String),

    #[error("[SerializationError] {0}")]
    Serialization(String),

    #[error("[SerializationError] {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("[InvalidArgument] {0}")]
    InvalidArgument(String),

    #[error("[Internal] {0}")]
    Internal(String),
}

/// Severity classification for error handling and logging.
///
/// Consumers can decide per-severity:
/// - `Critical`: abort the entire operation
/// - `Error`: skip this file, continue with next
/// - `Warning`: log and continue
/// - `Info`: no action needed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Critical,
    Error,
    Warning,
    Info,
}

impl ErrorSeverity {
    /// Returns true if this severity should abort the current batch/file.
    pub fn is_critical(self) -> bool {
        matches!(self, ErrorSeverity::Critical)
    }
}

impl CitadelError {
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            CitadelError::FatalPanic(_) => ErrorSeverity::Critical,
            CitadelError::StackOverflow { .. } => ErrorSeverity::Critical,
            CitadelError::DatabaseError(_) => ErrorSeverity::Critical,
            CitadelError::Migration(_) => ErrorSeverity::Critical,
            CitadelError::MigrationError(_) => ErrorSeverity::Critical,
            CitadelError::NotInitialized(_) => ErrorSeverity::Error,
            CitadelError::NotFound(_) => ErrorSeverity::Warning,
            CitadelError::ParseError(_) => ErrorSeverity::Error,
            CitadelError::UnsupportedSyntax(_) => ErrorSeverity::Info,
            CitadelError::InvalidSpan { .. } => ErrorSeverity::Error,
            CitadelError::TreeSitterError(_) => ErrorSeverity::Error,
            CitadelError::IoError(_) => ErrorSeverity::Error,
            CitadelError::Database(_) => ErrorSeverity::Error,
            CitadelError::Serialization(_) => ErrorSeverity::Error,
            CitadelError::SerializationError(_) => ErrorSeverity::Error,
            CitadelError::InvalidArgument(_) => ErrorSeverity::Error,
            CitadelError::Internal(_) => ErrorSeverity::Error,
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            CitadelError::ParseError(_) => "ParseError",
            CitadelError::UnsupportedSyntax(_) => "UnsupportedSyntax",
            CitadelError::FatalPanic(_) => "FatalPanic",
            CitadelError::StackOverflow { .. } => "StackOverflow",
            CitadelError::InvalidSpan { .. } => "InvalidSpan",
            CitadelError::TreeSitterError(_) => "TreeSitterError",
            CitadelError::IoError(_) => "IoError",
            CitadelError::DatabaseError(_) => "DatabaseError",
            CitadelError::Database(_) => "Database",
            CitadelError::Migration(_) => "Migration",
            CitadelError::MigrationError(_) => "MigrationError",
            CitadelError::NotInitialized(_) => "NotInitialized",
            CitadelError::NotFound(_) => "NotFound",
            CitadelError::Serialization(_) => "Serialization",
            CitadelError::SerializationError(_) => "SerializationError",
            CitadelError::InvalidArgument(_) => "InvalidArgument",
            CitadelError::Internal(_) => "Internal",
        }
    }
}

impl From<String> for CitadelError {
    fn from(s: String) -> Self {
        CitadelError::Internal(s)
    }
}

impl From<&str> for CitadelError {
    fn from(s: &str) -> Self {
        CitadelError::Internal(s.to_owned())
    }
}

impl From<rusqlite::Error> for CitadelError {
    fn from(e: rusqlite::Error) -> Self {
        CitadelError::DatabaseError(Box::new(e))
    }
}
