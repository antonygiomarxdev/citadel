// Forward-compatibility re-export of the new CitadelError for consumers
// that still use `StorageError`. New code should use `citadel_core::error::CitadelError`
// directly. This alias will be removed in citadel-core 0.3+.
pub use crate::error::CitadelError as StorageError;
