//! SecureStore backed by the Windows Credential Manager (generic credentials).
//!
//! Each value is stored as a `CRED_TYPE_GENERIC` credential whose target name
//! is `{app_identifier}/{key}`. Credentials are scoped to the current Windows
//! user and encrypted at rest by LSA/DPAPI. `CRED_PERSIST_LOCAL_MACHINE`
//! persists across reboots for this user profile (it does not survive an OS
//! reinstall, matching the "where supported" wording of the trait contract).

use windows::Win32::Foundation::ERROR_NOT_FOUND;
use windows::Win32::Security::Credentials::{
    CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW,
    CredDeleteW, CredFree, CredReadW, CredWriteW,
};
use windows::core::{HSTRING, PWSTR};

use super::Platform;
use crate::error::PlatformError;
use crate::traits::secure_store::SecureStore;

impl Platform {
    fn credential_target(&self, key: &str) -> Result<HSTRING, PlatformError> {
        if key.is_empty() {
            return Err(PlatformError::InvalidParameter(
                "secure store key must not be empty".to_string(),
            ));
        }
        let target = format!("{}/{}", self.app_identifier(), key);
        if target.encode_utf16().count() >= 32767 {
            // CRED_MAX_GENERIC_TARGET_NAME_LENGTH
            return Err(PlatformError::InvalidParameter(format!(
                "secure store key is too long: {key}"
            )));
        }
        Ok(HSTRING::from(target))
    }
}

impl SecureStore for Platform {
    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        let target = self.credential_target(key)?;
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        let result = unsafe { CredReadW(&target, CRED_TYPE_GENERIC, None, &mut credential) };
        match result {
            Ok(()) => {
                let value = unsafe {
                    let cred = &*credential;
                    if cred.CredentialBlob.is_null() || cred.CredentialBlobSize == 0 {
                        Vec::new()
                    } else {
                        std::slice::from_raw_parts(
                            cred.CredentialBlob,
                            cred.CredentialBlobSize as usize,
                        )
                        .to_vec()
                    }
                };
                unsafe { CredFree(credential.cast()) };
                Ok(Some(value))
            }
            Err(err) if err.code() == ERROR_NOT_FOUND.to_hresult() => Ok(None),
            Err(err) => Err(PlatformError::Platform(format!(
                "CredReadW failed for key {key}: {err}"
            ))),
        }
    }

    fn write(&self, key: &str, value: &[u8]) -> Result<(), PlatformError> {
        if value.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
            return Err(PlatformError::InvalidParameter(format!(
                "secure store value exceeds the Credential Manager blob limit \
                 ({} > {CRED_MAX_CREDENTIAL_BLOB_SIZE} bytes)",
                value.len()
            )));
        }
        let target = self.credential_target(key)?;
        // Shown as the "user name" column in the Credential Manager UI; it is
        // not used for lookups (the target name is the key).
        let user_name = HSTRING::from(self.app_identifier());
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_ptr().cast_mut()),
            CredentialBlobSize: value.len() as u32,
            CredentialBlob: value.as_ptr().cast_mut(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user_name.as_ptr().cast_mut()),
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0) }.map_err(|err| {
            PlatformError::Platform(format!("CredWriteW failed for key {key}: {err}"))
        })
    }

    fn delete(&self, key: &str) -> Result<(), PlatformError> {
        let target = self.credential_target(key)?;
        match unsafe { CredDeleteW(&target, CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            // Deleting an absent key is a no-op, matching the other backends.
            Err(err) if err.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(err) => Err(PlatformError::Platform(format!(
                "CredDeleteW failed for key {key}: {err}"
            ))),
        }
    }
}
