// crypto.rs
// End-to-end encryption for payload bytes before they touch disk or the network.
// Uses AES-256-GCM (authenticated encryption — integrity + confidentiality).
// The key is derived from a local secret stored in the OS keychain (Phase 1 TODO).

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};

/// Encrypt `plaintext` bytes.
/// Returns `nonce (12 bytes) ++ ciphertext` as a single Vec<u8>.
/// The nonce is prepended so decrypt() can extract it without extra state.
pub fn encrypt_payload(plaintext: &[u8], key_bytes: &[u8; 32]) -> Result<Vec<u8>, String> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| format!("encrypt error: {e}"))?;

    // Prepend the 12-byte nonce so decrypt() is self-contained
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt `nonce ++ ciphertext` produced by encrypt_payload().
pub fn decrypt_payload(blob: &[u8], key_bytes: &[u8; 32]) -> Result<Vec<u8>, String> {
    if blob.len() < 12 {
        return Err("blob too short to contain nonce".into());
    }
    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("decrypt error: {e}"))
}

/// Load or generate the 32-byte encryption key.
/// TODO (Phase 1): store/retrieve from OS keychain (keyring crate).
/// For now returns a fixed dev key — NEVER ship this.
pub fn load_or_create_key() -> [u8; 32] {
    // !! DEV PLACEHOLDER — replace with keychain integration !!
    b"sentinel-dev-key-replace-me-asap".clone()
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
}
