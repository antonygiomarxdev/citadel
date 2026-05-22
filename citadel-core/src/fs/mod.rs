use crate::error::CitadelError;
use std::path::Path;

/// Pure filesystem abstraction — the Dependency Inversion fix.
///
/// Instead of calling `std::fs::read_to_string` directly from domain logic
/// (violating Clean Architecture / DIP), consumers depend on this trait.
/// Production uses `RealFileSystem`; tests can inject a mock.
pub trait FileSystem: Send + Sync {
    /// Read the full contents of a file at `path`.
    fn read_to_string(&self, path: &Path) -> Result<String, CitadelError>;
    /// Check whether a file exists.
    fn exists(&self, path: &Path) -> bool;
    /// Create all parent directories for `path`, if they don't exist.
    fn create_dir_all(&self, path: &Path) -> Result<(), CitadelError>;
}

/// The real filesystem — delegates directly to `std::fs`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read_to_string(&self, path: &Path) -> Result<String, CitadelError> {
        std::fs::read_to_string(path).map_err(CitadelError::from)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), CitadelError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(CitadelError::from)?;
        }
        Ok(())
    }
}
