// crypto.rs
// End-to-end encryption for payload bytes before they touch disk or the network.
// Uses AES-256-GCM (authenticated encryption — integrity + confidentiality).
// The key is derived from a local secret stored in the OS keychain (Phase 1 TODO).

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};

use crate::error::SentinelError;
/// Encrypt `plaintext` bytes.
/// Returns `nonce (12 bytes) ++ ciphertext` as a single `Vec<u8>`.
/// The nonce is prepended so `decrypt_payload` is self-contained.
pub fn encrypt_payload(plaintext: &[u8], key_bytes: &[u8; 32]) -> Result<Vec<u8>, SentinelError> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| SentinelError::Crypto(e.to_string()))?;

    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt `nonce ++ ciphertext` produced by `encrypt_payload`.
pub fn decrypt_payload(blob: &[u8], key_bytes: &[u8; 32]) -> Result<Vec<u8>, SentinelError> {
    if blob.len() < 12 {
        return Err(SentinelError::Crypto(
            "blob too short to contain nonce".into(),
        ));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| SentinelError::Crypto(e.to_string()))
}

/// Load or generate the 32-byte encryption key.
///
/// TODO (Phase 1): store/retrieve from OS keychain (keyring crate).
/// For now returns a fixed dev key — NEVER ship this.
pub fn load_or_create_key() -> [u8; 32] {
    // 1. Bypass global state during testing to prevent concurrent race conditions
    #[cfg(test)]
    {
        return *b"sentinel-test-key-fixed-32-bytes";
    }

    // 2. Production keyring logic
    #[cfg(not(test))]
    {
        let entry = keyring::Entry::new("sentinel-gateway", "master-key")
            .expect("Failed to access system keyring");

        // Try to fetch existing key
        if let Ok(stored_key_hex) = entry.get_password() {
            if let Ok(decoded) = hex::decode(stored_key_hex) {
                if decoded.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&decoded);
                    return key;
                }
            }
        }

        // Generate and persist new key
        let mut new_key = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut new_key);

        let encoded = hex::encode(new_key);
        entry
            .set_password(&encoded)
            .expect("Failed to persist new key to keyring");

        new_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = load_or_create_key();
        let original = b"hello from IoT device";
        let blob = encrypt_payload(original, &key).unwrap();
        let recovered = decrypt_payload(&blob, &key).unwrap();
        assert_eq!(original.as_slice(), recovered.as_slice());
    }

    #[test]
    fn decrypt_rejects_short_blob() {
        let key = load_or_create_key();
        let err = decrypt_payload(&[0u8; 5], &key).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn encrypt_output_is_longer_than_input() {
        let key = load_or_create_key();
        let plaintext = b"test";
        let blob = encrypt_payload(plaintext, &key).unwrap();
        // nonce (12) + ciphertext (input + 16-byte GCM tag)
        assert!(blob.len() > plaintext.len() + 12);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key = load_or_create_key();
        let blob = encrypt_payload(b"secret", &key).unwrap();

        let mut bad_key = key;
        bad_key[0] ^= 0xFF;
        assert!(decrypt_payload(&blob, &bad_key).is_err());
    }
}
