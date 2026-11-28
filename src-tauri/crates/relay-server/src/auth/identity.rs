use ed25519_dalek::{Signature, VerifyingKey};
use together_crypto::encoding::array_from_base64;
use together_crypto::identity::{public_from_bytes, signature_from_bytes};

pub fn public_from_base64(text: &str) -> Option<VerifyingKey> {
    let bytes = array_from_base64::<32>(text)?;
    public_from_bytes(&bytes)
}

pub fn signature_from_base64(text: &str) -> Option<Signature> {
    let bytes = array_from_base64::<64>(text)?;
    Some(signature_from_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use together_crypto::encoding::to_base64;
    use together_crypto::identity::{keypair_from_phrase, sign_nonce};

    const PHRASE: &str =
        "legal winner thank year wave sausage worth useful legal winner thank yellow";

    #[test]
    fn key_and_signature_survive_the_wire() {
        let key = keypair_from_phrase(PHRASE).expect("фраза верная");
        let public = to_base64(&key.verifying_key().to_bytes());
        let signature = to_base64(&sign_nonce(&key, &[3u8; 32]).to_bytes());

        assert!(public_from_base64(&public).is_some());
        assert!(signature_from_base64(&signature).is_some());
    }

    #[test]
    fn wrong_length_is_rejected() {
        assert!(public_from_base64(&to_base64(&[1u8; 31])).is_none());
        assert!(signature_from_base64(&to_base64(&[1u8; 63])).is_none());
        assert!(public_from_base64("совсем не ключ").is_none());
    }
}
