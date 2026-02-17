//! Protocol messages.

use crate::crypto::{Commitment, EncryptedPreimage, PaymentHash};
use crate::games::GameAction;
use crate::protocol::{GameId, GameResult, Player};
use serde::{Deserialize, Serialize};

/// Hold invoice information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HoldInvoiceMessage {
    pub payment_hash: PaymentHash,
    pub amount_sat: u64,
    pub expiry_secs: u64,
}

/// Phase 3: Encrypted preimage exchange
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedPreimageExchange {
    pub game_id: GameId,
    pub player: Player,
    pub encrypted_preimage: EncryptedPreimage,
}

/// Phase 4: Commitment message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitMessage {
    pub game_id: GameId,
    pub player: Player,
    pub commitment: Commitment,
}

/// Phase 5: Reveal message to Oracle
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevealMessage {
    pub game_id: GameId,
    pub player: Player,
    pub action: GameAction,
    pub salt: crate::crypto::Salt,
    pub commit_a: Commitment,
    pub commit_b: Commitment,
}

/// Phase 6: Oracle's signed result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleResultMessage {
    pub game_id: GameId,
    pub game_type: crate::games::GameType,
    pub game_data: GameData,
    pub result: GameResult,
    #[serde(with = "signature_serde")]
    pub signature: [u8; 64],
}

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

/// All game data needed to verify the result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameData {
    pub action_a: GameAction,
    pub action_b: GameAction,
    /// Oracle's secret (for games that require it)
    pub oracle_secret: Option<OracleSecretData>,
}

/// Oracle's secret data for verification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleSecretData {
    pub secret_value: Vec<u8>,
    pub nonce: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{Preimage, Salt};
    use crate::games::RpsAction;

    #[test]
    fn test_message_serialization() {
        let commit_msg = CommitMessage {
            game_id: GameId::new(),
            player: Player::A,
            commitment: Commitment::new(b"Rock", &Salt::random()),
        };

        let json = serde_json::to_string(&commit_msg).unwrap();
        let deserialized: CommitMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(commit_msg.game_id, deserialized.game_id);
        assert_eq!(commit_msg.player, deserialized.player);
    }

    #[test]
    fn test_hold_invoice_message() {
        let preimage = Preimage::random();
        let msg = HoldInvoiceMessage {
            payment_hash: preimage.payment_hash(),
            amount_sat: 1000,
            expiry_secs: 3600,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: HoldInvoiceMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.payment_hash, deserialized.payment_hash);
        assert_eq!(msg.amount_sat, deserialized.amount_sat);
    }
}
