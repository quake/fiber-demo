//! Preimage and PaymentHash for hold invoices.

use blake2b_rs::Blake2bBuilder;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fmt;

/// CKB default hash personalization
const CKB_HASH_PERSONALIZATION: &[u8] = b"ckb-default-hash";

/// Create a CKB hash (Blake2b-256 with CKB personalization)
fn ckb_hash(data: &[u8]) -> [u8; 32] {
    let mut blake2b = Blake2bBuilder::new(32)
        .personal(CKB_HASH_PERSONALIZATION)
        .build();
    blake2b.update(data);
    let mut hash = [0u8; 32];
    blake2b.finalize(&mut hash);
    hash
}

/// 32-byte preimage, its hash is the payment_hash
#[derive(Clone, Serialize, Deserialize)]
pub struct Preimage([u8; 32]);

impl Preimage {
    /// Create a new random preimage
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse from hex string (with or without 0x prefix)
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string (with 0x prefix for Fiber RPC)
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.0))
    }

    /// Compute the payment hash (CKB Hash = Blake2b-256 with "ckb-default-hash" personalization)
    pub fn payment_hash(&self) -> PaymentHash {
        PaymentHash(ckb_hash(&self.0))
    }
}

impl fmt::Debug for Preimage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Preimage({})", hex::encode(&self.0[..8]))
    }
}

/// CKB Hash (Blake2b-256) of preimage
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaymentHash([u8; 32]);

impl PaymentHash {
    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse from hex string (with or without 0x prefix)
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let s = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string (with 0x prefix for Fiber RPC)
    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.0))
    }

    /// Verify that a preimage matches this hash
    pub fn verify(&self, preimage: &Preimage) -> bool {
        preimage.payment_hash() == *self
    }
}

impl fmt::Debug for PaymentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PaymentHash({})", hex::encode(&self.0[..8]))
    }
}

impl fmt::Display for PaymentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preimage_hash_roundtrip() {
        let preimage = Preimage::random();
        let hash = preimage.payment_hash();

        assert!(hash.verify(&preimage));
    }

    #[test]
    fn test_different_preimages_different_hashes() {
        let preimage1 = Preimage::random();
        let preimage2 = Preimage::random();

        assert_ne!(preimage1.payment_hash(), preimage2.payment_hash());
    }

    #[test]
    fn test_wrong_preimage_fails_verification() {
        let preimage1 = Preimage::random();
        let preimage2 = Preimage::random();
        let hash1 = preimage1.payment_hash();

        assert!(!hash1.verify(&preimage2));
    }
}
