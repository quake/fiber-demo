//! Commitment and Salt for commit-reveal scheme.

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Salt for commitment scheme
#[derive(Clone, Serialize, Deserialize)]
pub struct Salt([u8; 32]);

impl Salt {
    /// Create a new random salt
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Salt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Salt({})", hex::encode(&self.0[..8]))
    }
}

/// Commitment = H(action || salt)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Commitment([u8; 32]);

impl Commitment {
    /// Create a commitment from action bytes and salt
    pub fn new(action_bytes: &[u8], salt: &Salt) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(action_bytes);
        hasher.update(salt.as_bytes());
        let result = hasher.finalize();
        Self(result.into())
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Verify that the given action and salt produce this commitment
    pub fn verify(&self, action_bytes: &[u8], salt: &Salt) -> bool {
        *self == Self::new(action_bytes, salt)
    }
}

impl fmt::Debug for Commitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Commitment({})", hex::encode(&self.0[..8]))
    }
}

impl fmt::Display for Commitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_verification() {
        let action = b"Rock";
        let salt = Salt::random();
        let commitment = Commitment::new(action, &salt);

        assert!(commitment.verify(action, &salt));
    }

    #[test]
    fn test_different_actions_different_commitments() {
        let salt = Salt::random();
        let commitment1 = Commitment::new(b"Rock", &salt);
        let commitment2 = Commitment::new(b"Paper", &salt);

        assert_ne!(commitment1, commitment2);
    }

    #[test]
    fn test_different_salts_different_commitments() {
        let action = b"Rock";
        let salt1 = Salt::random();
        let salt2 = Salt::random();
        let commitment1 = Commitment::new(action, &salt1);
        let commitment2 = Commitment::new(action, &salt2);

        assert_ne!(commitment1, commitment2);
    }

    #[test]
    fn test_wrong_action_fails_verification() {
        let salt = Salt::random();
        let commitment = Commitment::new(b"Rock", &salt);

        assert!(!commitment.verify(b"Paper", &salt));
    }

    #[test]
    fn test_wrong_salt_fails_verification() {
        let action = b"Rock";
        let salt1 = Salt::random();
        let salt2 = Salt::random();
        let commitment = Commitment::new(action, &salt1);

        assert!(!commitment.verify(action, &salt2));
    }
}
