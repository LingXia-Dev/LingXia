use lingxia_error::Error;
pub trait SecureStore: Send + Sync + 'static {
    /// Read a persisted value from a secure, app-scoped store that survives reinstall where supported.
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        Err(Error::not_supported(format!(
            "SecureStore::read not implemented for key {}",
            key
        )))
    }

    /// Check whether a value exists in the secure store.
    fn contains(&self, key: &str) -> Result<bool, Error> {
        Ok(self.read(key)?.is_some())
    }

    /// Persist a value into the secure store.
    fn write(&self, key: &str, value: &[u8]) -> Result<(), Error> {
        let _ = (key, value);
        Err(Error::not_supported(
            "SecureStore::write not implemented".to_string(),
        ))
    }

    /// Delete a value from the secure store.
    fn delete(&self, key: &str) -> Result<(), Error> {
        Err(Error::not_supported(format!(
            "SecureStore::delete not implemented for key {}",
            key
        )))
    }
}
