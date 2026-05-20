//! macOS Keychain storage for per-paired-iPhone shared secrets.
//!
//! Why Keychain and not a file: shared secrets used to HMAC-authenticate
//! the iPhone companion must not sit in a JSON blob alongside non-sensitive
//! state. macOS Keychain encrypts at rest under the user's login keychain
//! and gates read access to the binary that created the item (no prompts
//! for our own reads; prompts only if another app tries to read them).
//!
//! Service name `com.niyora.breathing.companion`, account `client.<uuid>`.
//! The companion feature is macOS-only, so this module is too.

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

const SERVICE: &str = "com.niyora.breathing.companion";

/// OSStatus returned by Keychain APIs when an item is not present. Inline
/// because pulling in `security-framework-sys` just for this single i32 is
/// not worth the extra crate.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

fn account(client_id: &str) -> String {
    format!("client.{client_id}")
}

/// Store the shared secret for a paired phone. Overwrites any existing
/// entry for the same `client_id` (re-pair refreshes the secret).
pub fn store_secret(client_id: &str, secret: &[u8]) -> Result<(), String> {
    set_generic_password(SERVICE, &account(client_id), secret)
        .map_err(|e| format!("keychain store: {e}"))
}

/// Look up the shared secret for a paired phone. Returns `None` if no
/// entry exists, an error only if Keychain access itself failed.
pub fn load_secret(client_id: &str) -> Result<Option<Vec<u8>>, String> {
    match get_generic_password(SERVICE, &account(client_id)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
        Err(e) => Err(format!("keychain load: {e}")),
    }
}

/// Forget a paired phone's secret. Called by the Settings "Unpair" button.
/// A missing item is not an error · the post-condition is "no item left".
pub fn delete_secret(client_id: &str) -> Result<(), String> {
    match delete_generic_password(SERVICE, &account(client_id)) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
        Err(e) => Err(format!("keychain delete: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real Keychain round-trip. Only runs locally on macOS; we tag the test
    /// with a unique client_id so concurrent test runs don't collide.
    #[test]
    fn round_trips_a_secret() {
        let client_id = format!("test-{}", uuid::Uuid::new_v4());
        let secret = b"shhh-do-not-tell".to_vec();

        store_secret(&client_id, &secret).expect("store");
        let got = load_secret(&client_id).expect("load").expect("present");
        assert_eq!(got, secret);

        delete_secret(&client_id).expect("delete");
        assert!(load_secret(&client_id).expect("load after delete").is_none());
    }

    #[test]
    fn missing_load_returns_none() {
        let client_id = format!("never-stored-{}", uuid::Uuid::new_v4());
        assert!(load_secret(&client_id).expect("load").is_none());
    }

    #[test]
    fn delete_missing_is_ok() {
        let client_id = format!("never-stored-{}", uuid::Uuid::new_v4());
        delete_secret(&client_id).expect("delete missing is fine");
    }

    #[test]
    fn overwrite_replaces_existing_secret() {
        let client_id = format!("test-overwrite-{}", uuid::Uuid::new_v4());
        store_secret(&client_id, b"first").expect("store first");
        store_secret(&client_id, b"second").expect("overwrite");
        assert_eq!(
            load_secret(&client_id).expect("load").expect("present"),
            b"second"
        );
        delete_secret(&client_id).ok();
    }
}
