use std::time::{Duration, Instant};

use ed25519_dalek::{Signature, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use together_crypto::encoding::to_base64;
use together_crypto::identity::{verify_nonce, NONCE_BYTES};

use crate::wire::ErrorCode;

#[derive(Debug, Clone)]
pub struct Challenge {
    public: VerifyingKey,
    nonce: [u8; NONCE_BYTES],
    deadline: Instant,
}

impl Challenge {
    
    pub fn issue(public: VerifyingKey, ttl: Duration) -> Self {
        Self::issue_at(public, ttl, Instant::now())
    }

    
    pub fn issue_at(public: VerifyingKey, ttl: Duration, now: Instant) -> Self {
        let mut nonce = [0u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);

        Self {
            public,
            nonce,
            deadline: now + ttl,
        }
    }

    
    pub fn nonce_base64(&self) -> String {
        to_base64(&self.nonce)
    }

    
    pub fn public(&self) -> VerifyingKey {
        self.public
    }

    
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.deadline
    }

    
    pub fn verify(&self, signature: &Signature, now: Instant) -> Result<VerifyingKey, ErrorCode> {
        if self.expired(now) {
            return Err(ErrorCode::ChallengeExpired);
        }

        if verify_nonce(&self.public, &self.nonce, signature) {
            Ok(self.public)
        } else {
            Err(ErrorCode::BadSig)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use together_crypto::encoding::from_base64;
    use together_crypto::identity::{keypair_from_phrase, sign_nonce};

    const PHRASE: &str =
        "legal winner thank year wave sausage worth useful legal winner thank yellow";

    #[test]
    fn honest_signature_passes() {
        let key = keypair_from_phrase(PHRASE).expect("фраза верная");
        let now = Instant::now();
        let challenge = Challenge::issue_at(key.verifying_key(), Duration::from_secs(10), now);

        let nonce = from_base64(&challenge.nonce_base64()).expect("nonce читается");
        assert_eq!(nonce.len(), NONCE_BYTES);

        let signature = sign_nonce(&key, &nonce);
        assert!(challenge.verify(&signature, now).is_ok());
    }

    #[test]
    fn foreign_key_does_not_pass() {
        let key = keypair_from_phrase(PHRASE).expect("фраза верная");
        let other = keypair_from_phrase(
            "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
        )
        .expect("фраза верная");

        let now = Instant::now();
        let challenge = Challenge::issue_at(key.verifying_key(), Duration::from_secs(10), now);
        let nonce = from_base64(&challenge.nonce_base64()).expect("nonce читается");

        let signature = sign_nonce(&other, &nonce);
        assert_eq!(challenge.verify(&signature, now), Err(ErrorCode::BadSig));
    }

    #[test]
    fn expired_challenge_is_useless() {
        let key = keypair_from_phrase(PHRASE).expect("фраза верная");
        let now = Instant::now();
        let challenge = Challenge::issue_at(key.verifying_key(), Duration::from_secs(10), now);
        let nonce = from_base64(&challenge.nonce_base64()).expect("nonce читается");
        let signature = sign_nonce(&key, &nonce);

        let late = now + Duration::from_secs(11);
        assert_eq!(
            challenge.verify(&signature, late),
            Err(ErrorCode::ChallengeExpired)
        );
    }

    #[test]
    fn nonces_do_not_repeat() {
        let key = keypair_from_phrase(PHRASE).expect("фраза верная");
        let first = Challenge::issue(key.verifying_key(), Duration::from_secs(10));
        let second = Challenge::issue(key.verifying_key(), Duration::from_secs(10));
        assert_ne!(first.nonce_base64(), second.nonce_base64());
    }
}
