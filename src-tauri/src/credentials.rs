//! Windows Credential Manager integration for MIMIC Phase 2.
//! Provides safe local OS credential storage and retrieval via Win32 CredWriteW/CredReadW.
//! Plaintext secrets are never stored in project JSON, configuration files, or logs.

use serde::{Deserialize, Serialize};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::Security::Credentials::{
        CredDeleteW, CredEnumerateW, CredFree, CredReadW, CredWriteW, CREDENTIALW,
        CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    },
};

const CRED_TARGET_PREFIX: &str = "MIMIC:";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSummary {
    pub target_name: String,
    pub username: String,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialEntry {
    pub target_name: String,
    pub username: String,
    pub secret: String,
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn qualify_target(name: &str) -> String {
    let clean = name.trim();
    if clean.starts_with(CRED_TARGET_PREFIX) {
        clean.to_string()
    } else {
        format!("{CRED_TARGET_PREFIX}{clean}")
    }
}

pub fn save_credential(target_name: &str, username: &str, secret: &str) -> Result<(), String> {
    let qualified = qualify_target(target_name);
    let mut target_wide = to_wide(&qualified);
    let mut user_wide = to_wide(username.trim());
    let secret_bytes = secret.as_bytes();

    let cred = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target_wide.as_mut_ptr()),
        Comment: PWSTR::null(),
        LastWritten: windows::Win32::Foundation::FILETIME::default(),
        CredentialBlobSize: secret_bytes.len() as u32,
        CredentialBlob: secret_bytes.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: PWSTR::null(),
        UserName: PWSTR(user_wide.as_mut_ptr()),
    };

    unsafe {
        CredWriteW(&cred, 0).map_err(|e| format!("Failed to save credential into Windows Credential Manager: {e}"))
    }
}

pub fn get_credential(target_name: &str) -> Result<CredentialEntry, String> {
    let qualified = qualify_target(target_name);
    let target_wide = to_wide(&qualified);

    unsafe {
        let mut cred_ptr: *mut CREDENTIALW = std::ptr::null_mut();
        CredReadW(
            PCWSTR(target_wide.as_ptr()),
            CRED_TYPE_GENERIC,
            Some(0),
            &mut cred_ptr,
        )
        .map_err(|e| format!("Credential '{target_name}' not found in Windows Credential Manager: {e}"))?;

        if cred_ptr.is_null() {
            return Err(format!("Credential '{target_name}' returned null pointer"));
        }

        let cred = &*cred_ptr;
        let username = if !cred.UserName.is_null() {
            cred.UserName.to_string().unwrap_or_default()
        } else {
            String::new()
        };

        let secret = if !cred.CredentialBlob.is_null() && cred.CredentialBlobSize > 0 {
            let slice = std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize);
            String::from_utf8(slice.to_vec()).unwrap_or_else(|_| String::from_utf8_lossy(slice).to_string())
        } else {
            String::new()
        };

        CredFree(cred_ptr as *const _);

        Ok(CredentialEntry {
            target_name: target_name.trim().to_string(),
            username,
            secret,
        })
    }
}

pub fn delete_credential(target_name: &str) -> Result<(), String> {
    let qualified = qualify_target(target_name);
    let target_wide = to_wide(&qualified);

    unsafe {
        CredDeleteW(PCWSTR(target_wide.as_ptr()), CRED_TYPE_GENERIC, Some(0))
            .map_err(|e| format!("Failed to delete credential '{target_name}': {e}"))
    }
}

pub fn list_credentials() -> Result<Vec<CredentialSummary>, String> {
    let filter_wide = to_wide(&format!("{CRED_TARGET_PREFIX}*"));

    unsafe {
        let mut count: u32 = 0;
        let mut creds_ptr: *mut *mut CREDENTIALW = std::ptr::null_mut();

        // Enumerate credentials with MIMIC:* prefix
        let res = CredEnumerateW(
            PCWSTR(filter_wide.as_ptr()),
            None,
            &mut count,
            &mut creds_ptr,
        );

        if res.is_err() || creds_ptr.is_null() || count == 0 {
            return Ok(Vec::new());
        }

        let mut list = Vec::with_capacity(count as usize);
        let creds_slice = std::slice::from_raw_parts(creds_ptr, count as usize);

        for &ptr in creds_slice {
            if !ptr.is_null() {
                let cred = &*ptr;
                let raw_target = if !cred.TargetName.is_null() {
                    cred.TargetName.to_string().unwrap_or_default()
                } else {
                    String::new()
                };

                let clean_target = if raw_target.starts_with(CRED_TARGET_PREFIX) {
                    raw_target[CRED_TARGET_PREFIX.len()..].to_string()
                } else {
                    raw_target
                };

                let username = if !cred.UserName.is_null() {
                    cred.UserName.to_string().unwrap_or_default()
                } else {
                    String::new()
                };

                list.push(CredentialSummary {
                    target_name: clean_target,
                    username,
                    last_updated: format!("{}:{}", cred.LastWritten.dwHighDateTime, cred.LastWritten.dwLowDateTime),
                });
            }
        }

        CredFree(creds_ptr as *const _);
        Ok(list)
    }
}

/// Resolves a secret reference formatted as "vault:<target_name>"
pub fn resolve_secret_reference(secret_ref: &str) -> Result<String, String> {
    let trimmed = secret_ref.trim();
    let target_name = if let Some(stripped) = trimmed.strip_prefix("vault:") {
        stripped
    } else {
        trimmed
    };

    if target_name.is_empty() {
        return Err("Secret reference target name cannot be empty".into());
    }

    let entry = get_credential(target_name)?;
    if entry.secret.is_empty() {
        return Err(format!("Secret in vault for target '{target_name}' is empty"));
    }
    Ok(entry.secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_credential_manager_roundtrip() {
        let test_target = "test_mimic_unit_account";
        let test_user = "mimic_player_test";
        let test_pass = "S3cur3P@ssw0rd!#2026";

        // 1. Write
        save_credential(test_target, test_user, test_pass).expect("Writing to Windows Credential Manager should succeed");

        // 2. Read
        let entry = get_credential(test_target).expect("Reading from Windows Credential Manager should succeed");
        assert_eq!(entry.username, test_user);
        assert_eq!(entry.secret, test_pass);

        // 3. Resolve Reference
        let resolved = resolve_secret_reference(&format!("vault:{test_target}")).expect("Resolving vault ref should succeed");
        assert_eq!(resolved, test_pass);

        // 4. List
        let list = list_credentials().expect("Listing credentials should succeed");
        assert!(list.iter().any(|c| c.target_name == test_target), "Credential must appear in summary list");

        // 5. Delete
        delete_credential(test_target).expect("Deleting credential should succeed");

        // 6. Verify Deletion
        let deleted = get_credential(test_target);
        assert!(deleted.is_err(), "Deleted credential should no longer exist");
    }
}
