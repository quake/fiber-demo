//! SignaturePoint for adaptor signatures.
//!
//! sig_point = R + H(R || O || game_id || result) * O
//! where:
//!   R = Oracle's commitment point (nonce)
//!   O = Oracle's public key
//!   game_id = unique game identifier
//!   result = game outcome ("A wins", "B wins", "Draw")

use crate::protocol::GameId;
use secp256k1::{PublicKey, Scalar, Secp256k1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Signature point for adaptor signatures
/// sig_point = R + H(R || O || game_id || result) * O
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignaturePoint(#[serde(with = "pubkey_serde")] PublicKey);

mod pubkey_serde {
    use secp256k1::PublicKey;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(key: &PublicKey, s: S) -> Result<S::Ok, S::Error> {
        hex::encode(key.serialize()).serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PublicKey, D::Error> {
        let hex_str = String::deserialize(d)?;
        let bytes = hex::decode(&hex_str).map_err(serde::de::Error::custom)?;
        PublicKey::from_slice(&bytes).map_err(serde::de::Error::custom)
    }
}

impl SignaturePoint {
    /// Compute signature point: R + H(R || O || game_id || result) * O
    pub fn compute(
        oracle_pubkey: &PublicKey,
        commitment_point: &PublicKey, // R
        game_id: &GameId,
        result: &str,
    ) -> Self {
        let secp = Secp256k1::new();

        // Compute challenge: H(R || O || game_id || result)
        let mut hasher = Sha256::new();
        hasher.update(commitment_point.serialize());
        hasher.update(oracle_pubkey.serialize());
        hasher.update(game_id.as_bytes());
        hasher.update(result.as_bytes());
        let hash = hasher.finalize();

        // Convert to scalar
        let scalar = Scalar::from_be_bytes(hash.into()).expect("valid scalar from hash");

        // Compute H(...) * O
        let tweaked = oracle_pubkey
            .mul_tweak(&secp, &scalar)
            .expect("valid tweak");

        // Compute R + H(...) * O
        let combined = commitment_point.combine(&tweaked).expect("valid combine");

        Self(combined)
    }

    /// Get the underlying public key
    pub fn as_pubkey(&self) -> &PublicKey {
        &self.0
    }

    /// Get the serialized bytes of the point
    pub fn to_bytes(&self) -> [u8; 33] {
        self.0.serialize()
    }

    /// Compute H(sig_point) for XOR encryption/decryption
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.0.serialize());
        hasher.finalize().into()
    }
}

impl fmt::Debug for SignaturePoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SignaturePoint({})",
            hex::encode(&self.0.serialize()[..8])
        )
    }
}

/// Signature points for all possible game outcomes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignaturePoints {
    pub a_wins: SignaturePoint,
    pub b_wins: SignaturePoint,
    pub draw: SignaturePoint,
}

/// Compute signature points for all possible outcomes
pub fn compute_signature_points(
    oracle_pubkey: &PublicKey,
    commitment_point: &PublicKey, // R
    game_id: &GameId,
) -> SignaturePoints {
    SignaturePoints {
        a_wins: SignaturePoint::compute(oracle_pubkey, commitment_point, game_id, "A wins"),
        b_wins: SignaturePoint::compute(oracle_pubkey, commitment_point, game_id, "B wins"),
        draw: SignaturePoint::compute(oracle_pubkey, commitment_point, game_id, "Draw"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::SecretKey;

    fn generate_keypair() -> (SecretKey, PublicKey) {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        (secret_key, public_key)
    }

    #[test]
    fn test_signature_point_computation() {
        let (_, oracle_pubkey) = generate_keypair();
        let (_, commitment_point) = generate_keypair();
        let game_id = GameId::new();

        let sig_point =
            SignaturePoint::compute(&oracle_pubkey, &commitment_point, &game_id, "A wins");

        // Verify it's a valid point (33 bytes compressed)
        assert_eq!(sig_point.to_bytes().len(), 33);
    }

    #[test]
    fn test_different_results_different_points() {
        let (_, oracle_pubkey) = generate_keypair();
        let (_, commitment_point) = generate_keypair();
        let game_id = GameId::new();

        let points = compute_signature_points(&oracle_pubkey, &commitment_point, &game_id);

        assert_ne!(points.a_wins, points.b_wins);
        assert_ne!(points.a_wins, points.draw);
        assert_ne!(points.b_wins, points.draw);
    }

    #[test]
    fn test_deterministic_computation() {
        let (_, oracle_pubkey) = generate_keypair();
        let (_, commitment_point) = generate_keypair();
        let game_id = GameId::new();

        let point1 = SignaturePoint::compute(&oracle_pubkey, &commitment_point, &game_id, "A wins");
        let point2 = SignaturePoint::compute(&oracle_pubkey, &commitment_point, &game_id, "A wins");

        assert_eq!(point1, point2);
    }
}
