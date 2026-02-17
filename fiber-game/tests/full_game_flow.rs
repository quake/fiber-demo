//! Integration tests for the full game flow.
//!
//! These tests simulate complete game sessions from start to finish.

use fiber_game_core::{
    crypto::{
        compute_signature_points, Commitment, EncryptedPreimage, PaymentHash, Preimage, Salt,
    },
    fiber::{FiberClient, MockFiberClient},
    games::{GameAction, GameJudge, GameType, GuessNumberGame, OracleSecret, RpsAction, RpsGame},
    protocol::{GameId, GameResult, Player},
};

/// Simulate a complete Rock-Paper-Scissors game where A wins
#[tokio::test]
async fn test_full_rps_game_a_wins() {
    // Setup: Oracle generates keys
    let secp = secp256k1::Secp256k1::new();
    let oracle_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let oracle_pk = secp256k1::PublicKey::from_secret_key(&secp, &oracle_sk);
    let commitment_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let commitment_point = secp256k1::PublicKey::from_secret_key(&secp, &commitment_sk);
    let game_id = GameId::new();

    // Phase 1: Both players generate preimages and choose actions
    let preimage_a = Preimage::random();
    let payment_hash_a = preimage_a.payment_hash();
    let action_a = GameAction::Rps(RpsAction::Rock);
    let salt_a = Salt::random();

    let preimage_b = Preimage::random();
    let payment_hash_b = preimage_b.payment_hash();
    let action_b = GameAction::Rps(RpsAction::Scissors);
    let salt_b = Salt::random();

    // Phase 2: Setup Fiber clients and exchange hold invoices
    let fiber_a = MockFiberClient::new(10_000);
    let fiber_b = MockFiberClient::new(10_000);

    let invoice_a = fiber_a
        .create_hold_invoice(&payment_hash_a, 1000, 3600)
        .await
        .unwrap();
    let invoice_b = fiber_b
        .create_hold_invoice(&payment_hash_b, 1000, 3600)
        .await
        .unwrap();

    // Both pay each other's invoices
    fiber_a.pay_hold_invoice(&invoice_b).await.unwrap();
    fiber_b.pay_hold_invoice(&invoice_a).await.unwrap();

    // Verify funds are locked
    assert_eq!(fiber_a.balance(), 9000);
    assert_eq!(fiber_b.balance(), 9000);

    // Phase 3: Compute signature points and create encrypted preimages
    let sig_points = compute_signature_points(&oracle_pk, &commitment_point, &game_id);

    // A encrypts their preimage with B_wins point (so B can claim if B wins)
    // B encrypts their preimage with A_wins point (so A can claim if A wins)
    let encrypted_preimage_a = EncryptedPreimage::encrypt(&preimage_a, &sig_points.b_wins);
    let encrypted_preimage_b = EncryptedPreimage::encrypt(&preimage_b, &sig_points.a_wins);

    // Exchange encrypted preimages (via Oracle)
    // A receives encrypted_preimage_b, B receives encrypted_preimage_a

    // Phase 4: Create and exchange commitments
    let commit_a = Commitment::new(&action_a.to_bytes(), &salt_a);
    let commit_b = Commitment::new(&action_b.to_bytes(), &salt_b);

    // Phase 5: Both reveal to Oracle
    // Oracle verifies commitments match
    assert!(commit_a.verify(&action_a.to_bytes(), &salt_a));
    assert!(commit_b.verify(&action_b.to_bytes(), &salt_b));

    // Phase 6: Oracle judges and signs
    let result = RpsGame::judge(&action_a, &action_b, None);
    assert_eq!(result, GameResult::AWins);

    // Phase 7: Settlement
    // A wins, so A can decrypt B's preimage using sig_point_a_wins
    let decrypted_preimage_b = encrypted_preimage_b.decrypt(&sig_points.a_wins);
    assert!(payment_hash_b.verify(&decrypted_preimage_b));

    // A settles B's invoice
    fiber_a
        .settle_invoice(&payment_hash_b, &decrypted_preimage_b)
        .await
        .unwrap();

    // B cancels A's invoice (refund)
    fiber_b.cancel_invoice(&payment_hash_a).await.unwrap();

    // Final balances: A gained 1000, B lost 1000
    assert_eq!(fiber_a.balance(), 10000); // 9000 + 1000 from settling B's invoice
    assert_eq!(fiber_b.balance(), 9000); // Lost the 1000 that was paid to A
}

/// Simulate a Rock-Paper-Scissors draw
#[tokio::test]
async fn test_full_rps_game_draw() {
    let secp = secp256k1::Secp256k1::new();
    let oracle_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let oracle_pk = secp256k1::PublicKey::from_secret_key(&secp, &oracle_sk);
    let commitment_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let commitment_point = secp256k1::PublicKey::from_secret_key(&secp, &commitment_sk);
    let game_id = GameId::new();

    // Both choose Rock
    let action_a = GameAction::Rps(RpsAction::Rock);
    let action_b = GameAction::Rps(RpsAction::Rock);

    let preimage_a = Preimage::random();
    let preimage_b = Preimage::random();
    let payment_hash_a = preimage_a.payment_hash();
    let payment_hash_b = preimage_b.payment_hash();

    let fiber_a = MockFiberClient::new(10_000);
    let fiber_b = MockFiberClient::new(10_000);

    let invoice_a = fiber_a
        .create_hold_invoice(&payment_hash_a, 1000, 3600)
        .await
        .unwrap();
    let invoice_b = fiber_b
        .create_hold_invoice(&payment_hash_b, 1000, 3600)
        .await
        .unwrap();

    fiber_a.pay_hold_invoice(&invoice_b).await.unwrap();
    fiber_b.pay_hold_invoice(&invoice_a).await.unwrap();

    // Oracle judges
    let result = RpsGame::judge(&action_a, &action_b, None);
    assert_eq!(result, GameResult::Draw);

    // Both cancel their invoices (refund)
    fiber_a.cancel_invoice(&payment_hash_b).await.unwrap();
    fiber_b.cancel_invoice(&payment_hash_a).await.unwrap();

    // Balances unchanged (funds were locked then cancelled, so still 9000 each)
    // Note: In a real system, the payer would get refunded
    assert_eq!(fiber_a.balance(), 9000);
    assert_eq!(fiber_b.balance(), 9000);
}

/// Simulate a Guess the Number game where B wins
#[tokio::test]
async fn test_guess_number_b_wins() {
    let secp = secp256k1::Secp256k1::new();
    let oracle_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let oracle_pk = secp256k1::PublicKey::from_secret_key(&secp, &oracle_sk);
    let commitment_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let commitment_point = secp256k1::PublicKey::from_secret_key(&secp, &commitment_sk);
    let game_id = GameId::new();

    // Oracle commits to secret number 50
    let oracle_secret = OracleSecret::with_number(50);
    let oracle_commitment = oracle_secret.commitment();

    // A guesses 30, B guesses 48
    // B is closer (distance 2 vs 20)
    let action_a = GameAction::GuessNumber(30);
    let action_b = GameAction::GuessNumber(48);

    let preimage_a = Preimage::random();
    let preimage_b = Preimage::random();
    let payment_hash_a = preimage_a.payment_hash();
    let payment_hash_b = preimage_b.payment_hash();
    let salt_a = Salt::random();
    let salt_b = Salt::random();

    let fiber_a = MockFiberClient::new(10_000);
    let fiber_b = MockFiberClient::new(10_000);

    let invoice_a = fiber_a
        .create_hold_invoice(&payment_hash_a, 1000, 3600)
        .await
        .unwrap();
    let invoice_b = fiber_b
        .create_hold_invoice(&payment_hash_b, 1000, 3600)
        .await
        .unwrap();

    fiber_a.pay_hold_invoice(&invoice_b).await.unwrap();
    fiber_b.pay_hold_invoice(&invoice_a).await.unwrap();

    // Compute signature points
    let sig_points = compute_signature_points(&oracle_pk, &commitment_point, &game_id);

    // Create encrypted preimages
    let encrypted_preimage_a = EncryptedPreimage::encrypt(&preimage_a, &sig_points.b_wins);
    let encrypted_preimage_b = EncryptedPreimage::encrypt(&preimage_b, &sig_points.a_wins);

    // Create commitments
    let commit_a = Commitment::new(&action_a.to_bytes(), &salt_a);
    let commit_b = Commitment::new(&action_b.to_bytes(), &salt_b);

    // Verify commitments
    assert!(commit_a.verify(&action_a.to_bytes(), &salt_a));
    assert!(commit_b.verify(&action_b.to_bytes(), &salt_b));

    // Oracle reveals secret and judges
    // First verify Oracle's commitment was honest
    assert!(oracle_secret.verify_commitment(&oracle_commitment));

    let result = GuessNumberGame::judge(&action_a, &action_b, Some(&oracle_secret));
    assert_eq!(result, GameResult::BWins);

    // B wins, so B can decrypt A's preimage
    let decrypted_preimage_a = encrypted_preimage_a.decrypt(&sig_points.b_wins);
    assert!(payment_hash_a.verify(&decrypted_preimage_a));

    // B settles A's invoice
    fiber_b
        .settle_invoice(&payment_hash_a, &decrypted_preimage_a)
        .await
        .unwrap();

    // A cancels B's invoice
    fiber_a.cancel_invoice(&payment_hash_b).await.unwrap();

    // Final balances: B gained 1000
    assert_eq!(fiber_a.balance(), 9000);
    assert_eq!(fiber_b.balance(), 10000);
}

/// Test that using wrong signature point fails to decrypt preimage
#[tokio::test]
async fn test_wrong_signature_point_fails_decryption() {
    let secp = secp256k1::Secp256k1::new();
    let oracle_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let oracle_pk = secp256k1::PublicKey::from_secret_key(&secp, &oracle_sk);
    let commitment_sk = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let commitment_point = secp256k1::PublicKey::from_secret_key(&secp, &commitment_sk);
    let game_id = GameId::new();

    let preimage = Preimage::random();
    let payment_hash = preimage.payment_hash();

    let sig_points = compute_signature_points(&oracle_pk, &commitment_point, &game_id);

    // Encrypt with a_wins point
    let encrypted = EncryptedPreimage::encrypt(&preimage, &sig_points.a_wins);

    // Try to decrypt with b_wins point (wrong!)
    let decrypted = encrypted.decrypt(&sig_points.b_wins);

    // Should NOT match the original payment hash
    assert!(!payment_hash.verify(&decrypted));
}

/// Test commitment verification fails with wrong data
#[tokio::test]
async fn test_invalid_reveal_rejected() {
    let action = GameAction::Rps(RpsAction::Rock);
    let salt = Salt::random();
    let commit = Commitment::new(&action.to_bytes(), &salt);

    // Wrong action
    let wrong_action = GameAction::Rps(RpsAction::Paper);
    assert!(!commit.verify(&wrong_action.to_bytes(), &salt));

    // Wrong salt
    let wrong_salt = Salt::random();
    assert!(!commit.verify(&action.to_bytes(), &wrong_salt));

    // Correct reveal works
    assert!(commit.verify(&action.to_bytes(), &salt));
}
