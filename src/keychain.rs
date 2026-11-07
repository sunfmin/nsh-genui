//! The user's DeepSeek API key, kept in the macOS login keychain. Only the host reads it: the shell's JS sees
//! whether one is set and its last four characters, the sandbox nothing.

use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

const SERVICE: &str = "dev.felix.nsh-genui";
const ACCOUNT: &str = "deepseek-api-key";
/// `errSecItemNotFound`.
const NOT_FOUND: i32 = -25300;

pub fn get() -> Result<Option<String>, String> {
    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| "the stored key is not text".into()),
        Err(e) if e.code() == NOT_FOUND => Ok(None),
        Err(e) => Err(format!("keychain: {e}")),
    }
}

pub fn set(key: &str) -> Result<(), String> {
    set_generic_password(SERVICE, ACCOUNT, key.as_bytes()).map_err(|e| match e.code() {
        WRITE_PERMISSION => "钥匙串不让这个进程写入：从 SSH 启动的进程用不了登录钥匙串。请打开 GenUI.app（scripts/bundle.sh 生成）".into(),
        _ => format!("keychain: {e}"),
    })
}

/// `errSecWrPerm`: what a process outside the user's login session (started over SSH) gets.
const WRITE_PERMISSION: i32 = -61;

pub fn delete() -> Result<(), String> {
    match delete_generic_password(SERVICE, ACCOUNT) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == NOT_FOUND => Ok(()),
        Err(e) => Err(format!("keychain: {e}")),
    }
}

/// Writes, reads and removes an item of its own: whether this process may use the keychain (a process
/// started over SSH may not; one LaunchServices starts in the user's session may). For `src/dev.rs`.
pub fn check() -> Result<(), String> {
    const TEST: &str = "check-only";
    set_generic_password(SERVICE, TEST, b"sk-check").map_err(|e| format!("write: {e}"))?;
    let read = get_generic_password(SERVICE, TEST).map_err(|e| format!("read: {e}"))?;
    delete_generic_password(SERVICE, TEST).map_err(|e| format!("delete: {e}"))?;
    if read == b"sk-check" { Ok(()) } else { Err("read back something else".into()) }
}

/// `sk-…abcd`: enough to recognize a key, not to use it.
pub fn hint(key: &str) -> String {
    let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{}…{tail}", key.chars().take(3).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Writes the login keychain (an item of its own, removed at the end): `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn a_key_is_kept_read_and_removed() {
        const TEST: &str = "test-only";
        set_generic_password(SERVICE, TEST, b"sk-test").unwrap();
        assert_eq!(get_generic_password(SERVICE, TEST).unwrap(), b"sk-test");
        delete_generic_password(SERVICE, TEST).unwrap();
        assert_eq!(get_generic_password(SERVICE, TEST).unwrap_err().code(), NOT_FOUND);
        assert_eq!(hint("sk-abcdefgh1234"), "sk-…1234");
    }
}
