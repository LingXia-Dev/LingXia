use crate::error::PlatformError;

pub trait SecureStore: Send + Sync + 'static {
    /// Read a persisted value from a secure, app-scoped store that survives reinstall where supported.
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        Err(PlatformError::NotSupported(format!(
            "SecureStore::read not implemented for key {}",
            key
        )))
    }

    /// Check whether a value exists in the secure store.
    fn contains(&self, key: &str) -> Result<bool, PlatformError> {
        Ok(self.read(key)?.is_some())
    }

    /// Persist a value into the secure store.
    fn write(&self, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        let _ = (key, value);
        Err(PlatformError::NotSupported(
            "SecureStore::write not implemented".to_string(),
        ))
    }

    /// Delete a value from the secure store.
    fn delete(&self, key: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(format!(
            "SecureStore::delete not implemented for key {}",
            key
        )))
    }
}
