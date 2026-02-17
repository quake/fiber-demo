//! Encrypted preimage using adaptor signature scheme.
//!
//! encrypted_preimage = preimage XOR H(sig_point)
//!
//! The winner can decrypt this using the Oracle's actual signature.

use super::{Preimage, SignaturePoint};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Encrypted preimage = preimage XOR H(sig_point)
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedPreimage([u8; 32]);

impl EncryptedPreimage {
    /// Encrypt preimage with signature point
    /// encrypted = preimage XOR H(sig_point)
    pub fn encrypt(preimage: &Preimage, sig_point: &SignaturePoint) -> Self {
        let mask = sig_point.hash();
        let mut result = [0u8; 32];
        for i in 0..32 {
            result[i] = preimage.as_bytes()[i] ^ mask[i];
        }
        Self(result)
    }

    /// Decrypt using the signature point derived from Oracle's actual signature
    /// preimage = encrypted XOR H(sig_point)
    pub fn decrypt(&self, sig_point: &SignaturePoint) -> Preimage {
        let mask = sig_point.hash();
        let mut result = [0u8; 32];
        for i in 0..32 {
            result[i] = self.0[i] ^ mask[i];
        }
        Preimage::from_bytes(result)
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

impl fmt::Debug for EncryptedPreimage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncryptedPreimage({})", hex::encode(&self.0[..8]))
    }
}

/// Oracle's signature on game result
/// This can be used to derive the signature point for decryption
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleSignature {
    /// The actual signature bytes (Schnorr signature) as hex string
    #[serde(with = "signature_serde")]
    pub signature: [u8; 64],
    /// The signed message
    pub message: Vec<u8>,
}

#[allow(dead_code)]
mod signature_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        hex::encode(bytes).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let hex_str = String::deserialize(d)?;
        let bytes = hex::decode(&hex_str).map_err(serde::de::Error::custom)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom("expected 64 bytes"));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

#[allow(dead_code)]
impl OracleSignature {
    /// Extract the signature point from the Oracle's signature
    /// For a Schnorr signature (R, s), the signature point we computed was:
    /// sig_point = R + H(R || O || game_id || result) * O
    ///
    /// After Oracle signs, we can verify and use the signature to decrypt.
    /// In a real implementation, we would extract R from the signature
    /// and combine it with the challenge to get the same point.
    pub fn derive_signature_point(
        &self,
        oracle_pubkey: &secp256k1::PublicKey,
        commitment_point: &secp256k1::PublicKey,
        game_id: &crate::protocol::GameId,
        result: &str,
    ) -> SignaturePoint {
        // In the actual protocol, the Oracle uses a specific nonce (commitment_point)
        // and we compute the same signature point that was used for encryption
        SignaturePoint::compute(oracle_pubkey, commitment_point, game_id, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::compute_signature_points;
    use crate::protocol::GameId;
    use secp256k1::{PublicKey, Secp256k1, SecretKey};

    fn generate_keypair() -> (SecretKey, PublicKey) {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        (secret_key, public_key)
    }

    #[test]
    fn test_encrypted_preimage_encrypt_decrypt() {
        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let (_, oracle_pubkey) = generate_keypair();
        let (_, commitment_point) = generate_keypair();
        let game_id = GameId::new();

        let points = compute_signature_points(&oracle_pubkey, &commitment_point, &game_id);

        // Player B encrypts their preimage with sig_point_A_wins
        // (so A can decrypt it when A wins)
        let encrypted = EncryptedPreimage::encrypt(&preimage, &points.a_wins);

        // Simulate A winning and getting the signature point
        // A decrypts using the same signature point
        let decrypted = encrypted.decrypt(&points.a_wins);

        // Verify the decrypted preimage is correct
        assert!(payment_hash.verify(&decrypted));
        assert_eq!(preimage.as_bytes(), decrypted.as_bytes());
    }

    #[test]
    fn test_wrong_signature_point_fails() {
        let preimage = Preimage::random();
        let payment_hash = preimage.payment_hash();

        let (_, oracle_pubkey) = generate_keypair();
        let (_, commitment_point) = generate_keypair();
        let game_id = GameId::new();

        let points = compute_signature_points(&oracle_pubkey, &commitment_point, &game_id);

        // Encrypt with a_wins point
        let encrypted = EncryptedPreimage::encrypt(&preimage, &points.a_wins);

        // Try to decrypt with b_wins point (wrong!)
        let decrypted = encrypted.decrypt(&points.b_wins);

        // Should NOT verify
        assert!(!payment_hash.verify(&decrypted));
    }

    #[test]
    fn test_encryption_is_symmetric() {
        let preimage = Preimage::random();

        let (_, oracle_pubkey) = generate_keypair();
        let (_, commitment_point) = generate_keypair();
        let game_id = GameId::new();

        let sig_point =
            SignaturePoint::compute(&oracle_pubkey, &commitment_point, &game_id, "A wins");

        // Encrypt
        let encrypted = EncryptedPreimage::encrypt(&preimage, &sig_point);

        // XOR is symmetric, so encrypting the encrypted value should give back original
        let double_encrypted =
            EncryptedPreimage::encrypt(&Preimage::from_bytes(*encrypted.as_bytes()), &sig_point);

        assert_eq!(preimage.as_bytes(), double_encrypted.as_bytes());
    }
}
