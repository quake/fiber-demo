//! Rock-Paper-Scissors game implementation.

use super::traits::{GameAction, GameJudge};
use super::OracleSecret;
use crate::protocol::GameResult;
use serde::{Deserialize, Serialize};

/// Rock-Paper-Scissors action
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpsAction {
    Rock,
    Paper,
    Scissors,
}

impl RpsAction {
    /// Convert to bytes for commitment
    pub fn to_bytes(&self) -> &[u8] {
        match self {
            RpsAction::Rock => b"Rock",
            RpsAction::Paper => b"Paper",
            RpsAction::Scissors => b"Scissors",
        }
    }

    /// Check if this action beats the other
    pub fn beats(&self, other: &RpsAction) -> bool {
        matches!(
            (self, other),
            (RpsAction::Rock, RpsAction::Scissors)
                | (RpsAction::Scissors, RpsAction::Paper)
                | (RpsAction::Paper, RpsAction::Rock)
        )
    }
}

/// Rock-Paper-Scissors game
pub struct RpsGame;

impl GameJudge for RpsGame {
    fn judge(
        action_a: &GameAction,
        action_b: &GameAction,
        _oracle_secret: Option<&OracleSecret>,
    ) -> GameResult {
        let (rps_a, rps_b) = match (action_a, action_b) {
            (GameAction::Rps(a), GameAction::Rps(b)) => (a, b),
            _ => panic!("Invalid action type for RPS game"),
        };

        if rps_a == rps_b {
            GameResult::Draw
        } else if rps_a.beats(rps_b) {
            GameResult::AWins
        } else {
            GameResult::BWins
        }
    }

    fn validate_action(action: &GameAction) -> bool {
        matches!(action, GameAction::Rps(_))
    }

    fn requires_oracle_secret() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judge_rps(a: RpsAction, b: RpsAction) -> GameResult {
        RpsGame::judge(&GameAction::Rps(a), &GameAction::Rps(b), None)
    }

    #[test]
    fn test_rps_rock_beats_scissors() {
        assert_eq!(
            judge_rps(RpsAction::Rock, RpsAction::Scissors),
            GameResult::AWins
        );
        assert_eq!(
            judge_rps(RpsAction::Scissors, RpsAction::Rock),
            GameResult::BWins
        );
    }

    #[test]
    fn test_rps_scissors_beats_paper() {
        assert_eq!(
            judge_rps(RpsAction::Scissors, RpsAction::Paper),
            GameResult::AWins
        );
        assert_eq!(
            judge_rps(RpsAction::Paper, RpsAction::Scissors),
            GameResult::BWins
        );
    }

    #[test]
    fn test_rps_paper_beats_rock() {
        assert_eq!(
            judge_rps(RpsAction::Paper, RpsAction::Rock),
            GameResult::AWins
        );
        assert_eq!(
            judge_rps(RpsAction::Rock, RpsAction::Paper),
            GameResult::BWins
        );
    }

    #[test]
    fn test_rps_draws() {
        assert_eq!(
            judge_rps(RpsAction::Rock, RpsAction::Rock),
            GameResult::Draw
        );
        assert_eq!(
            judge_rps(RpsAction::Paper, RpsAction::Paper),
            GameResult::Draw
        );
        assert_eq!(
            judge_rps(RpsAction::Scissors, RpsAction::Scissors),
            GameResult::Draw
        );
    }

    #[test]
    fn test_rps_all_outcomes() {
        // All 9 combinations
        let actions = [RpsAction::Rock, RpsAction::Paper, RpsAction::Scissors];
        let mut a_wins = 0;
        let mut b_wins = 0;
        let mut draws = 0;

        for a in &actions {
            for b in &actions {
                match judge_rps(*a, *b) {
                    GameResult::AWins => a_wins += 1,
                    GameResult::BWins => b_wins += 1,
                    GameResult::Draw => draws += 1,
                }
            }
        }

        assert_eq!(a_wins, 3); // Rock>Scissors, Scissors>Paper, Paper>Rock
        assert_eq!(b_wins, 3); // Symmetric
        assert_eq!(draws, 3); // Rock=Rock, Paper=Paper, Scissors=Scissors
    }

    #[test]
    fn test_rps_validate_action() {
        assert!(RpsGame::validate_action(&GameAction::Rps(RpsAction::Rock)));
        assert!(RpsGame::validate_action(&GameAction::Rps(RpsAction::Paper)));
        assert!(RpsGame::validate_action(&GameAction::Rps(
            RpsAction::Scissors
        )));
        assert!(!RpsGame::validate_action(&GameAction::GuessNumber(50)));
    }

    #[test]
    fn test_rps_no_oracle_secret() {
        assert!(!RpsGame::requires_oracle_secret());
    }
}
