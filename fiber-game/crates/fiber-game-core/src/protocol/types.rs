//! Protocol types.

use secp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Unique game identifier
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(Uuid);

impl GameId {
    /// Create a new random game ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get bytes representation for hashing
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Default for GameId {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for GameId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl fmt::Debug for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GameId({})", self.0)
    }
}

impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Game result
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameResult {
    AWins,
    BWins,
    Draw,
}

impl GameResult {
    /// Convert to string for signature computation
    pub fn as_str(&self) -> &'static str {
        match self {
            GameResult::AWins => "A wins",
            GameResult::BWins => "B wins",
            GameResult::Draw => "Draw",
        }
    }
}

impl fmt::Display for GameResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Player identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Player {
    A,
    B,
}

impl Player {
    /// Get the opponent
    pub fn opponent(&self) -> Player {
        match self {
            Player::A => Player::B,
            Player::B => Player::A,
        }
    }
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Player::A => write!(f, "A"),
            Player::B => write!(f, "B"),
        }
    }
}

/// Game session information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameSession {
    /// Unique game identifier
    pub game_id: GameId,
    /// Type of game
    pub game_type: crate::games::GameType,
    /// Oracle's public key
    #[serde(with = "pubkey_serde")]
    pub oracle_pubkey: PublicKey,
    /// Oracle's commitment point (R) for this game
    #[serde(with = "pubkey_serde")]
    pub oracle_commitment_point: PublicKey,
    /// Oracle's commitment hash (for games requiring Oracle secret)
    pub oracle_commitment: Option<[u8; 32]>,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_id_generation() {
        let id1 = GameId::new();
        let id2 = GameId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_player_opponent() {
        assert_eq!(Player::A.opponent(), Player::B);
        assert_eq!(Player::B.opponent(), Player::A);
    }

    #[test]
    fn test_game_result_str() {
        assert_eq!(GameResult::AWins.as_str(), "A wins");
        assert_eq!(GameResult::BWins.as_str(), "B wins");
        assert_eq!(GameResult::Draw.as_str(), "Draw");
    }
}
