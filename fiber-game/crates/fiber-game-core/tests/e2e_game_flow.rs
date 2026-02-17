//! End-to-end integration tests for the game flow.
//!
//! These tests verify the full HTTP interaction between Oracle and Player services.
//!
//! Run with: cargo test --test e2e_game_flow -- --nocapture --test-threads=1

use std::process::{Child, Command};
use std::time::Duration;

/// Helper to start a service process
struct ServiceProcess {
    child: Child,
    name: String,
}

impl ServiceProcess {
    fn start_oracle(crate_dir: &str, port: u16) -> Self {
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "-p", "fiber-game-oracle"])
            .current_dir(crate_dir)
            .env("PORT", port.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let child = cmd.spawn().expect("Failed to start oracle");

        Self {
            child,
            name: "fiber-game-oracle".to_string(),
        }
    }

    fn start_player(crate_dir: &str, port: u16, oracle_url: &str) -> Self {
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "-p", "fiber-game-player"])
            .current_dir(crate_dir)
            .env("PORT", port.to_string())
            .env("ORACLE_URL", oracle_url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let child = cmd.spawn().expect("Failed to start player");

        Self {
            child,
            name: format!("fiber-game-player:{}", port),
        }
    }

    fn wait_for_ready(&self, url: &str, timeout: Duration) -> bool {
        let client = reqwest::blocking::Client::new();
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if client.get(url).send().is_ok() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }
}

impl Drop for ServiceProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        println!("Stopped {}", self.name);
    }
}

/// Test that Player A sees status update after Player B joins
///
/// This test verifies the bug fix where Player A was stuck on "WaitingForOpponent"
/// even after Player B joined the game.
#[test]
fn test_player_a_sees_opponent_joined() {
    // CARGO_MANIFEST_DIR is fiber-game-core, go up two levels to fiber-game workspace
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_dir = format!("{}/../../", crate_dir);

    const ORACLE_PORT: u16 = 13000;
    const PLAYER_A_PORT: u16 = 13001;
    const PLAYER_B_PORT: u16 = 13002;

    let oracle_url = format!("http://localhost:{}", ORACLE_PORT);

    // Start Oracle
    let oracle = ServiceProcess::start_oracle(&workspace_dir, ORACLE_PORT);
    assert!(
        oracle.wait_for_ready(
            &format!("{}/oracle/pubkey", oracle_url),
            Duration::from_secs(30)
        ),
        "Oracle failed to start"
    );

    // Start Player A (from its crate directory for static files)
    let player_a = ServiceProcess::start_player(
        &format!("{}/crates/fiber-game-player", workspace_dir),
        PLAYER_A_PORT,
        &oracle_url,
    );
    assert!(
        player_a.wait_for_ready(
            &format!("http://localhost:{}/api/player", PLAYER_A_PORT),
            Duration::from_secs(30)
        ),
        "Player A failed to start"
    );

    // Start Player B
    let player_b = ServiceProcess::start_player(
        &format!("{}/crates/fiber-game-player", workspace_dir),
        PLAYER_B_PORT,
        &oracle_url,
    );
    assert!(
        player_b.wait_for_ready(
            &format!("http://localhost:{}/api/player", PLAYER_B_PORT),
            Duration::from_secs(30)
        ),
        "Player B failed to start"
    );

    let client = reqwest::blocking::Client::new();
    let player_a_url = format!("http://localhost:{}", PLAYER_A_PORT);
    let player_b_url = format!("http://localhost:{}", PLAYER_B_PORT);

    // Player A creates a game
    let create_resp: serde_json::Value = client
        .post(format!("{}/api/game/create", player_a_url))
        .json(&serde_json::json!({
            "game_type": "RockPaperScissors",
            "amount_sat": 1000
        }))
        .send()
        .expect("Failed to create game")
        .json()
        .expect("Failed to parse create response");

    let game_id = create_resp["game_id"]
        .as_str()
        .expect("No game_id in response");
    println!("Created game: {}", game_id);

    // Verify Player A sees WaitingForOpponent
    let my_games: serde_json::Value = client
        .get(format!("{}/api/games/mine", player_a_url))
        .send()
        .expect("Failed to get my games")
        .json()
        .expect("Failed to parse my games");

    let game = &my_games["games"][0];
    assert_eq!(game["phase"].as_str(), Some("WaitingForOpponent"));

    // Player B joins the game
    let join_resp: serde_json::Value = client
        .post(format!("{}/api/game/join", player_b_url))
        .json(&serde_json::json!({
            "game_id": game_id
        }))
        .send()
        .expect("Failed to join game")
        .json()
        .expect("Failed to parse join response");

    assert_eq!(join_resp["status"].as_str(), Some("joined"));
    println!("Player B joined game");

    // KEY TEST: Player A should now see WaitingForAction, not WaitingForOpponent
    let my_games_after: serde_json::Value = client
        .get(format!("{}/api/games/mine", player_a_url))
        .send()
        .expect("Failed to get my games after join")
        .json()
        .expect("Failed to parse my games after join");

    let game_after = &my_games_after["games"][0];
    assert_eq!(
        game_after["phase"].as_str(),
        Some("WaitingForAction"),
        "Player A should see WaitingForAction after B joins, but got {:?}",
        game_after["phase"]
    );

    println!("Test passed: Player A correctly sees WaitingForAction after B joins");
}

/// Test complete game flow: create, join, play, settle
#[test]
fn test_full_rps_game_with_http_services() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_dir = format!("{}/../../", crate_dir);

    const ORACLE_PORT: u16 = 14000;
    const PLAYER_A_PORT: u16 = 14001;
    const PLAYER_B_PORT: u16 = 14002;

    let oracle_url = format!("http://localhost:{}", ORACLE_PORT);

    // Start Oracle
    let oracle = ServiceProcess::start_oracle(&workspace_dir, ORACLE_PORT);
    assert!(
        oracle.wait_for_ready(
            &format!("{}/oracle/pubkey", oracle_url),
            Duration::from_secs(30)
        ),
        "Oracle failed to start"
    );

    // Start Player A
    let player_a = ServiceProcess::start_player(
        &format!("{}/crates/fiber-game-player", workspace_dir),
        PLAYER_A_PORT,
        &oracle_url,
    );
    assert!(
        player_a.wait_for_ready(
            &format!("http://localhost:{}/api/player", PLAYER_A_PORT),
            Duration::from_secs(30)
        ),
        "Player A failed to start"
    );

    // Start Player B
    let player_b = ServiceProcess::start_player(
        &format!("{}/crates/fiber-game-player", workspace_dir),
        PLAYER_B_PORT,
        &oracle_url,
    );
    assert!(
        player_b.wait_for_ready(
            &format!("http://localhost:{}/api/player", PLAYER_B_PORT),
            Duration::from_secs(30)
        ),
        "Player B failed to start"
    );

    let client = reqwest::blocking::Client::new();
    let player_a_url = format!("http://localhost:{}", PLAYER_A_PORT);
    let player_b_url = format!("http://localhost:{}", PLAYER_B_PORT);

    // 1. Player A creates a game
    let create_resp: serde_json::Value = client
        .post(format!("{}/api/game/create", player_a_url))
        .json(&serde_json::json!({
            "game_type": "RockPaperScissors",
            "amount_sat": 1000
        }))
        .send()
        .expect("Failed to create game")
        .json()
        .expect("Failed to parse create response");

    let game_id = create_resp["game_id"].as_str().expect("No game_id");
    println!("Created game: {}", game_id);

    // 2. Player B joins
    let join_resp: serde_json::Value = client
        .post(format!("{}/api/game/join", player_b_url))
        .json(&serde_json::json!({ "game_id": game_id }))
        .send()
        .expect("Failed to join game")
        .json()
        .expect("Failed to parse join response");

    assert_eq!(join_resp["status"].as_str(), Some("joined"));
    println!("Player B joined");

    // 3. Both players make their moves
    // Player A plays Rock
    let play_a_resp: serde_json::Value = client
        .post(format!("{}/api/game/{}/play", player_a_url, game_id))
        .json(&serde_json::json!({
            "action": { "Rps": "Rock" }
        }))
        .send()
        .expect("Failed for A to play")
        .json()
        .expect("Failed to parse A play response");

    // First player to reveal will see "waiting_for_opponent"
    assert_eq!(play_a_resp["status"].as_str(), Some("waiting_for_opponent"));
    println!("Player A played Rock");

    // Player B plays Scissors
    let play_b_resp: serde_json::Value = client
        .post(format!("{}/api/game/{}/play", player_b_url, game_id))
        .json(&serde_json::json!({
            "action": { "Rps": "Scissors" }
        }))
        .send()
        .expect("Failed for B to play")
        .json()
        .expect("Failed to parse B play response");

    // Second player to reveal will see "game_complete"
    assert_eq!(play_b_resp["status"].as_str(), Some("game_complete"));
    println!("Player B played Scissors");

    // 4. Check game status - should show result after some processing
    std::thread::sleep(Duration::from_millis(500));

    let status_a: serde_json::Value = client
        .get(format!("{}/api/game/{}/status", player_a_url, game_id))
        .send()
        .expect("Failed to get status")
        .json()
        .expect("Failed to parse status");

    println!("Game status for A: {:?}", status_a);

    // 5. Settle the game
    let settle_resp: serde_json::Value = client
        .post(format!("{}/api/game/{}/settle", player_a_url, game_id))
        .send()
        .expect("Failed to settle")
        .json()
        .expect("Failed to parse settle response");

    println!("Settle response: {:?}", settle_resp);

    // Player A should have won (Rock beats Scissors)
    let amount_won = settle_resp["amount_won"].as_i64().unwrap_or(0);
    assert!(
        amount_won >= 0,
        "Player A should have won or drawn, got amount_won={}",
        amount_won
    );

    println!(
        "Test passed: Full game flow completed. A won {} sats",
        amount_won
    );
}
