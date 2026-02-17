//! Guess the Number game implementation.

use super::traits::{GameAction, GameJudge};
use crate::protocol::GameResult;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Oracle's secret for Guess the Number game
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleSecret {
    /// The secret number (0-99)
    pub secret_number: u8,
    /// Random nonce for commitment
    pub nonce: [u8; 32],
}

impl OracleSecret {
    /// Generate a new random Oracle secret
    pub fn random() -> Self {
        let mut nonce = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);
        let secret_number = (rand::random::<u8>()) % 100;
        Self {
            secret_number,
            nonce,
        }
    }

    /// Create with a specific secret number
    pub fn with_number(secret_number: u8) -> Self {
        assert!(secret_number < 100, "Secret number must be 0-99");
        let mut nonce = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);
        Self {
            secret_number,
            nonce,
        }
    }

    /// Compute commitment: H(secret_number || nonce)
    pub fn commitment(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update([self.secret_number]);
        hasher.update(&self.nonce);
        hasher.finalize().into()
    }

    /// Verify that this secret matches a commitment
    pub fn verify_commitment(&self, commitment: &[u8; 32]) -> bool {
        &self.commitment() == commitment
    }
}

/// Guess the Number game
pub struct GuessNumberGame;

impl GuessNumberGame {
    /// Calculate distance from guess to secret number
    fn distance(guess: u8, secret: u8) -> u8 {
        if guess > secret {
            guess - secret
        } else {
            secret - guess
        }
    }
}

impl GameJudge for GuessNumberGame {
    fn judge(
        action_a: &GameAction,
        action_b: &GameAction,
        oracle_secret: Option<&OracleSecret>,
    ) -> GameResult {
        let (guess_a, guess_b) = match (action_a, action_b) {
            (GameAction::GuessNumber(a), GameAction::GuessNumber(b)) => (*a, *b),
            _ => panic!("Invalid action type for GuessNumber game"),
        };

        let secret = oracle_secret
            .expect("GuessNumber game requires Oracle secret")
            .secret_number;

        let distance_a = Self::distance(guess_a, secret);
        let distance_b = Self::distance(guess_b, secret);

        if distance_a < distance_b {
            GameResult::AWins
        } else if distance_b < distance_a {
            GameResult::BWins
        } else {
            GameResult::Draw
        }
    }

    fn validate_action(action: &GameAction) -> bool {
        matches!(action, GameAction::GuessNumber(n) if *n < 100)
    }

    fn requires_oracle_secret() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judge_guess(a: u8, b: u8, secret: u8) -> GameResult {
        let oracle_secret = OracleSecret::with_number(secret);
        GuessNumberGame::judge(
            &GameAction::GuessNumber(a),
            &GameAction::GuessNumber(b),
            Some(&oracle_secret),
        )
    }

    #[test]
    fn test_guess_number_closer_wins() {
        // Secret is 50
        // A guesses 48 (distance 2), B guesses 55 (distance 5)
        // A is closer, A wins
        assert_eq!(judge_guess(48, 55, 50), GameResult::AWins);
    }

    #[test]
    fn test_guess_number_b_wins() {
        // Secret is 50
        // A guesses 30 (distance 20), B guesses 45 (distance 5)
        // B is closer, B wins
        assert_eq!(judge_guess(30, 45, 50), GameResult::BWins);
    }

    #[test]
    fn test_guess_number_tie() {
        // Secret is 50
        // A guesses 45 (distance 5), B guesses 55 (distance 5)
        // Same distance, Draw
        assert_eq!(judge_guess(45, 55, 50), GameResult::Draw);
    }

    #[test]
    fn test_guess_number_exact_guess() {
        // Secret is 50
        // A guesses exactly 50 (distance 0), B guesses 51 (distance 1)
        // A wins
        assert_eq!(judge_guess(50, 51, 50), GameResult::AWins);
    }

    #[test]
    fn test_guess_number_both_exact() {
        // Secret is 50
        // Both guess exactly 50
        // Draw
        assert_eq!(judge_guess(50, 50, 50), GameResult::Draw);
    }

    #[test]
    fn test_guess_number_edge_cases() {
        // Secret is 0
        assert_eq!(judge_guess(0, 1, 0), GameResult::AWins);
        assert_eq!(judge_guess(5, 3, 0), GameResult::BWins);

        // Secret is 99
        assert_eq!(judge_guess(99, 98, 99), GameResult::AWins);
        assert_eq!(judge_guess(90, 95, 99), GameResult::BWins);
    }

    #[test]
    fn test_oracle_secret_commitment_verification() {
        let secret = OracleSecret::random();
        let commitment = secret.commitment();

        assert!(secret.verify_commitment(&commitment));
    }

    #[test]
    fn test_oracle_secret_wrong_commitment_fails() {
        let secret1 = OracleSecret::random();
        let secret2 = OracleSecret::random();

        let commitment1 = secret1.commitment();

        // Different secret should not verify against first commitment
        assert!(!secret2.verify_commitment(&commitment1));
    }

    #[test]
    fn test_guess_number_validate_action() {
        assert!(GuessNumberGame::validate_action(&GameAction::GuessNumber(
            0
        )));
        assert!(GuessNumberGame::validate_action(&GameAction::GuessNumber(
            50
        )));
        assert!(GuessNumberGame::validate_action(&GameAction::GuessNumber(
            99
        )));
        assert!(!GuessNumberGame::validate_action(&GameAction::GuessNumber(
            100
        )));
        assert!(!GuessNumberGame::validate_action(&GameAction::Rps(
            crate::games::RpsAction::Rock
        )));
    }

    #[test]
    fn test_guess_number_requires_oracle_secret() {
        assert!(GuessNumberGame::requires_oracle_secret());
    }
}
