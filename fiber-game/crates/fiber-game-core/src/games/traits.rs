//! Game traits and types.

use crate::protocol::GameResult;
use serde::{Deserialize, Serialize};

/// Type of game
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameType {
    RockPaperScissors,
    GuessNumber,
}

impl GameType {
    /// Does this game require Oracle to commit a secret beforehand?
    pub fn requires_oracle_secret(&self) -> bool {
        match self {
            GameType::RockPaperScissors => false,
            GameType::GuessNumber => true,
        }
    }
}

/// Game-specific action
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameAction {
    Rps(super::RpsAction),
    GuessNumber(u8), // 0-99
}

impl GameAction {
    /// Convert to bytes for commitment
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            GameAction::Rps(action) => action.to_bytes().to_vec(),
            GameAction::GuessNumber(n) => vec![*n],
        }
    }

    /// Validate that this action is legal for the given game type
    pub fn validate(&self, game_type: GameType) -> bool {
        match (self, game_type) {
            (GameAction::Rps(_), GameType::RockPaperScissors) => true,
            (GameAction::GuessNumber(n), GameType::GuessNumber) => *n < 100,
            _ => false,
        }
    }
}

/// Trait for game logic - each game type implements this
pub trait GameJudge {
    /// Determine winner from actions and optional Oracle secret
    fn judge(
        action_a: &GameAction,
        action_b: &GameAction,
        oracle_secret: Option<&super::OracleSecret>,
    ) -> GameResult;

    /// Validate that an action is legal for this game
    fn validate_action(action: &GameAction) -> bool;

    /// Does this game require Oracle to commit a secret beforehand?
    fn requires_oracle_secret() -> bool;
}
