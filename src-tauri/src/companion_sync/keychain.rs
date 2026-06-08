//! macOS Keychain storage for the companion's Noise identities.
//!
//! Two things live here:
//!
//! - The Mac's own long-term static private key (its Noise identity). One
//!   per Mac, created on first use and reused forever so paired phones keep
//!   recognising this Mac across launches.
//! - Each paired phone's static *public* key, keyed by `client_id`. On
//!   reconnect the Mac compares the key the phone presents in the handshake
//!   against the stored one, so a phone cannot reconnect as another's
//!   `client_id`. A public key is not itself a secret, but it is identity
//!   data and belongs in the Keychain next to the private key, not in a
//!   JSON blob.
//!
//! macOS Keychain encrypts at rest under the user's login keychain and gates
//! read access to the binary that created the item. Service name
//! `com.niyora.breathing.companion`. The companion feature is macOS-only,
//! so this module is too.

use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

const SERVICE: &str = "com.niyora.breathing.companion";

/// Fixed account holding this Mac's 32-byte Noise static private key.
const MAC_IDENTITY_ACCOUNT: &str = "mac.noise_static_private";

/// OSStatus returned by Keychain APIs when an item is not present. Inline
/// because pulling in `security-framework-sys` just for this single i32 is
/// not worth the extra crate.
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

fn account(client_id: &str) -> String {
    format!("client.{client_id}")
}

/// Load this Mac's Noise static private key, creating and persisting one on
/// first use. The returned 32 bytes are passed to the handshake responder;
/// snow derives the matching public key from them, so only the private key
/// needs storing.
pub fn load_or_create_mac_identity() -> Result<Vec<u8>, String> {
    match get_generic_password(SERVICE, MAC_IDENTITY_ACCOUNT) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => {
            let kp = super::noise::generate_static_keypair()?;
            set_generic_password(SERVICE, MAC_IDENTITY_ACCOUNT, &kp.private)
                .map_err(|e| format!("keychain store mac identity: {e}"))?;
            Ok(kp.private)
        }
        Err(e) => Err(format!("keychain load mac identity: {e}")),
    }
}

/// Store a paired phone's static public key. Overwrites any existing entry
/// for the same `client_id` (re-pair refreshes the identity).
pub fn store_peer_pubkey(client_id: &str, pubkey: &[u8]) -> Result<(), String> {
    set_generic_password(SERVICE, &account(client_id), pubkey)
        .map_err(|e| format!("keychain store: {e}"))
}

/// Look up a paired phone's static public key. Returns `None` if no entry
/// exists, an error only if Keychain access itself failed.
pub fn load_peer_pubkey(client_id: &str) -> Result<Option<Vec<u8>>, String> {
    match get_generic_password(SERVICE, &account(client_id)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
        Err(e) => Err(format!("keychain load: {e}")),
    }
}

/// Forget a paired phone's key. Called by the Settings "Unpair" button.
/// A missing item is not an error · the post-condition is "no item left".
pub fn delete_peer_pubkey(client_id: &str) -> Result<(), String> {
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
    fn round_trips_a_peer_pubkey() {
        let client_id = format!("test-{}", uuid::Uuid::new_v4());
        let pubkey = vec![7u8; 32];

        store_peer_pubkey(&client_id, &pubkey).expect("store");
        let got = load_peer_pubkey(&client_id).expect("load").expect("present");
        assert_eq!(got, pubkey);

        delete_peer_pubkey(&client_id).expect("delete");
        assert!(load_peer_pubkey(&client_id)
            .expect("load after delete")
            .is_none());
    }

    #[test]
    fn missing_load_returns_none() {
        let client_id = format!("never-stored-{}", uuid::Uuid::new_v4());
        assert!(load_peer_pubkey(&client_id).expect("load").is_none());
    }

    #[test]
    fn delete_missing_is_ok() {
        let client_id = format!("never-stored-{}", uuid::Uuid::new_v4());
        delete_peer_pubkey(&client_id).expect("delete missing is fine");
    }

    #[test]
    fn overwrite_replaces_existing_pubkey() {
        let client_id = format!("test-overwrite-{}", uuid::Uuid::new_v4());
        store_peer_pubkey(&client_id, &[1u8; 32]).expect("store first");
        store_peer_pubkey(&client_id, &[2u8; 32]).expect("overwrite");
        assert_eq!(
            load_peer_pubkey(&client_id).expect("load").expect("present"),
            vec![2u8; 32]
        );
        delete_peer_pubkey(&client_id).ok();
    }

    #[test]
    fn mac_identity_is_stable_across_calls() {
        let a = load_or_create_mac_identity().expect("first");
        let b = load_or_create_mac_identity().expect("second");
        assert_eq!(a, b, "the Mac identity must persist, not regenerate");
        assert_eq!(a.len(), 32);
    }
}
