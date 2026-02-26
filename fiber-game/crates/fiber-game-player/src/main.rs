//! Fiber Game Player Service
//!
//! HTTP service with Web UI for players to create/join games and play.
//! All Fiber RPC calls are made by the frontend directly — the backend
//! only handles game state management and Oracle communication.

use axum::{
    extract::{Path, State},
    http::{self, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use fiber_game_core::{
    crypto::{Commitment, EncryptedPreimage, PaymentHash, Preimage, Salt},
    games::{GameAction, GameType},
    protocol::{GameId, GameResult, Player},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

/// Application error type
struct AppError(String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, self.0).into_response()
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError(s.to_string())
    }
}

/// Player state
struct PlayerState {
    player_id: Uuid,
    player_name: String,
    oracle_url: String,
    http_client: Client,
    /// Fiber RPC URL for this player's node (configured via env var, exposed to frontend)
    fiber_rpc_url: Option<String>,
    games: RwLock<HashMap<GameId, PlayerGameState>>,
}

/// State of a game from player's perspective
#[derive(Clone)]
#[allow(dead_code)]
struct PlayerGameState {
    role: Player,
    game_type: GameType,
    amount_shannons: u64,
    /// My preimage (only I know this, used to settle opponent's invoice if I win)
    preimage: Preimage,
    /// My payment_hash = H(preimage), shared with opponent
    payment_hash: PaymentHash,
    /// Opponent's payment_hash (used to create my invoice that opponent pays)
    opponent_payment_hash: Option<PaymentHash>,
    /// Opponent's preimage (revealed by Oracle if I win, used to settle my_invoice)
    opponent_preimage: Option<Preimage>,
    salt: Salt,
    action: Option<GameAction>,
    oracle_pubkey: Option<secp256k1::PublicKey>,
    commitment_point: Option<secp256k1::PublicKey>,
    opponent_encrypted_preimage: Option<EncryptedPreimage>,
    my_commitment: Option<Commitment>,
    opponent_commitment: Option<Commitment>,
    opponent_action: Option<GameAction>,
    phase: PlayerGamePhase,
    result: Option<GameResult>,
    /// My invoice string (created by frontend on my Fiber node)
    my_invoice_string: Option<String>,
    /// Opponent's invoice string (retrieved from Oracle, paid by frontend)
    opponent_invoice_string: Option<String>,
    /// Whether the frontend has reported paying opponent's invoice
    paid_opponent: bool,
    /// Oracle's secret number for Guess Number games (revealed with result)
    oracle_secret_number: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum PlayerGamePhase {
    WaitingForOpponent,
    ExchangingInvoices,
    ExchangingEncryptedPreimages,
    WaitingForAction,
    Committed,
    Revealed,
    WaitingForResult,
    Settled,
}

// === Request/Response types ===

#[derive(Serialize)]
struct PlayerInfoResponse {
    player_id: Uuid,
    player_name: String,
    fiber_rpc_url: Option<String>,
}

#[derive(Serialize)]
struct AvailableGameResponse {
    game_id: GameId,
    game_type: GameType,
    amount_shannons: u64,
}

#[derive(Serialize)]
struct AvailableGamesResponse {
    games: Vec<AvailableGameResponse>,
}

#[derive(Serialize)]
struct MyGameResponse {
    game_id: GameId,
    game_type: GameType,
    role: Player,
    phase: PlayerGamePhase,
    amount_shannons: u64,
    result: Option<GameResult>,
}

#[derive(Serialize)]
struct MyGamesResponse {
    games: Vec<MyGameResponse>,
}

#[derive(Deserialize)]
struct CreateGameRequest {
    game_type: GameType,
    amount_shannons: u64,
}

#[derive(Serialize)]
struct CreateGameResponse {
    game_id: GameId,
}

#[derive(Deserialize)]
struct JoinGameRequest {
    game_id: GameId,
}

#[derive(Serialize)]
struct JoinGameResponse {
    status: String,
}

#[derive(Deserialize)]
struct PlayRequest {
    action: GameAction,
}

#[derive(Serialize)]
struct PlayResponse {
    status: String,
}

#[derive(Serialize)]
struct GameStatusResponse {
    role: Player,
    phase: PlayerGamePhase,
    result: Option<GameResult>,
    my_action: Option<GameAction>,
    opponent_action: Option<GameAction>,
    can_settle: bool,
    /// Opponent's payment_hash (hex) — frontend uses this to create hold invoice
    opponent_payment_hash: Option<String>,
    /// Opponent's preimage (hex) — revealed by Oracle if this player won, used to settle
    opponent_preimage: Option<String>,
    /// My payment_hash (hex) — needed for settle/cancel
    my_payment_hash: Option<String>,
    /// Oracle's secret number for Guess Number games
    #[serde(skip_serializing_if = "Option::is_none")]
    oracle_secret_number: Option<u8>,
}

#[derive(Serialize)]
struct SettleResponse {
    result: GameResult,
    amount_won: i64,
}

/// Request from frontend reporting that it created an invoice on its Fiber node
#[derive(Deserialize)]
struct InvoiceCreatedRequest {
    invoice_string: String,
}

/// Request from frontend reporting that it paid the opponent's invoice
#[derive(Deserialize)]
struct PaymentDoneRequest {
    // placeholder for future fields if needed
}

#[derive(Serialize)]
struct InvoiceCreatedResponse {
    status: String,
}

#[derive(Serialize)]
struct PaymentDoneResponse {
    status: String,
}

impl PlayerState {
    fn new(player_id: Uuid, player_name: String, oracle_url: String, fiber_rpc_url: Option<String>) -> Self {
        Self {
            player_id,
            player_name,
            oracle_url,
            http_client: Client::new(),
            fiber_rpc_url,
            games: RwLock::new(HashMap::new()),
        }
    }
}

// === Route handlers ===

async fn get_player_info(State(state): State<Arc<PlayerState>>) -> Result<Json<PlayerInfoResponse>, AppError> {
    Ok(Json(PlayerInfoResponse {
        player_id: state.player_id,
        player_name: state.player_name.clone(),
        fiber_rpc_url: state.fiber_rpc_url.clone(),
    }))
}

async fn get_available_games(
    State(state): State<Arc<PlayerState>>,
) -> Result<Json<AvailableGamesResponse>, AppError> {
    let url = format!("{}/games/available", state.oracle_url);
    let resp: serde_json::Value = state
        .http_client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError(e.to_string()))?;

    // Get the set of game IDs this player has already joined/created
    let my_game_ids: std::collections::HashSet<GameId> = {
        let games = state.games.read().unwrap();
        games.keys().copied().collect()
    };

    // Filter out games that this player created
    let games: Vec<AvailableGameResponse> = resp["games"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|g| {
            let game_id: GameId = serde_json::from_value(g["game_id"].clone()).ok()?;
            // Skip games this player already has
            if my_game_ids.contains(&game_id) {
                return None;
            }
            Some(AvailableGameResponse {
                game_id,
                game_type: serde_json::from_value(g["game_type"].clone()).ok()?,
                amount_shannons: g["amount_shannons"].as_u64().unwrap_or(0),
            })
        })
        .collect();

    Ok(Json(AvailableGamesResponse { games }))
}

async fn get_my_games(State(state): State<Arc<PlayerState>>) -> Json<MyGamesResponse> {
    // Check Oracle for games waiting for opponent
    let games_to_check: Vec<(GameId, u64)> = {
        let games = state.games.read().unwrap();
        games
            .iter()
            .filter(|(_, g)| g.phase == PlayerGamePhase::WaitingForOpponent)
            .map(|(id, g)| (*id, g.amount_shannons))
            .collect()
    };

    // Update phase for games where opponent has joined
    for (game_id, _amount) in games_to_check {
        let url = format!("{}/game/{}/status", state.oracle_url, game_id);
        if let Ok(resp) = state.http_client.get(&url).send().await {
            if let Ok(status_data) = resp.json::<serde_json::Value>().await {
                if status_data["has_opponent"].as_bool() == Some(true) {
                    // Get opponent's (B's) payment_hash so frontend can create invoice
                    let get_hash_url = format!("{}/game/{}/payment-hash/B", state.oracle_url, game_id);
                    if let Ok(hash_resp) = state.http_client.get(&get_hash_url).send().await {
                        if hash_resp.status().is_success() {
                            if let Ok(hash_data) = hash_resp.json::<serde_json::Value>().await {
                                if let Some(hash_array) = hash_data["payment_hash"].as_array() {
                                    let hash_bytes: Vec<u8> = hash_array
                                        .iter()
                                        .map(|v| v.as_u64().unwrap_or(0) as u8)
                                        .collect();

                                    if let Ok(hash_arr) = <[u8; 32]>::try_from(hash_bytes.as_slice()) {
                                        let opponent_payment_hash = PaymentHash::from_bytes(hash_arr);

                                        let mut games = state.games.write().unwrap();
                                        if let Some(game) = games.get_mut(&game_id) {
                                            game.opponent_payment_hash = Some(opponent_payment_hash);
                                            // Transition to WaitingForAction — frontend will
                                            // handle invoice creation via Fiber RPC
                                            game.phase = PlayerGamePhase::WaitingForAction;
                                        }

                                        info!("{}: Opponent joined game {:?}, got opponent payment_hash", state.player_name, game_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let games = state.games.read().unwrap();
    let my_games: Vec<MyGameResponse> = games
        .iter()
        .map(|(id, g)| MyGameResponse {
            game_id: *id,
            game_type: g.game_type,
            role: g.role,
            phase: g.phase,
            amount_shannons: g.amount_shannons,
            result: g.result,
        })
        .collect();

    Json(MyGamesResponse { games: my_games })
}

async fn create_game(
    State(state): State<Arc<PlayerState>>,
    Json(req): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, AppError> {
    let url = format!("{}/game/create", state.oracle_url);

    let body = serde_json::json!({
        "game_type": req.game_type,
        "player_a_id": state.player_id,
        "amount_shannons": req.amount_shannons,
    });

    let resp: serde_json::Value = state
        .http_client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError(e.to_string()))?;

    let game_id: GameId = serde_json::from_value(resp["game_id"].clone())
        .map_err(|e| AppError(e.to_string()))?;

    let oracle_pubkey = hex::decode(resp["oracle_pubkey"].as_str().unwrap_or(""))
        .ok()
        .and_then(|b| secp256k1::PublicKey::from_slice(&b).ok());

    let commitment_point = hex::decode(resp["commitment_point"].as_str().unwrap_or(""))
        .ok()
        .and_then(|b| secp256k1::PublicKey::from_slice(&b).ok());

    let preimage = Preimage::random();
    let payment_hash = preimage.payment_hash();
    let salt = Salt::random();

    // Submit payment_hash to Oracle immediately so opponent can get it when they join
    let submit_hash_url = format!("{}/game/{}/payment-hash", state.oracle_url, game_id);
    let submit_hash_body = serde_json::json!({
        "player": Player::A,
        "payment_hash": payment_hash,
        "preimage": preimage,
    });

    state.http_client
        .post(&submit_hash_url)
        .json(&submit_hash_body)
        .send()
        .await
        .map_err(|e| AppError(format!("Failed to submit payment hash: {}", e)))?;

    info!("{}: Submitted payment_hash to Oracle for game {:?}", state.player_name, game_id);

    let game_state = PlayerGameState {
        role: Player::A,
        game_type: req.game_type,
        amount_shannons: req.amount_shannons,
        preimage,
        payment_hash,
        opponent_payment_hash: None,
        opponent_preimage: None,
        salt,
        action: None,
        oracle_pubkey,
        commitment_point,
        opponent_encrypted_preimage: None,
        my_commitment: None,
        opponent_commitment: None,
        opponent_action: None,
        phase: PlayerGamePhase::WaitingForOpponent,
        result: None,
        my_invoice_string: None,
        opponent_invoice_string: None,
        paid_opponent: false,
        oracle_secret_number: None,
    };

    state.games.write().unwrap().insert(game_id, game_state);

    info!("{}: Created game {:?}", state.player_name, game_id);

    Ok(Json(CreateGameResponse { game_id }))
}

async fn join_game(
    State(state): State<Arc<PlayerState>>,
    Json(req): Json<JoinGameRequest>,
) -> Result<Json<JoinGameResponse>, AppError> {
    let url = format!("{}/game/{}/join", state.oracle_url, req.game_id);
    info!("{}: Joining game {:?}, calling {}", state.player_name, req.game_id, url);

    let body = serde_json::json!({
        "player_b_id": state.player_id,
    });

    let response = state
        .http_client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error!("{}: Failed to send join request: {}", state.player_name, e);
            AppError(e.to_string())
        })?;

    let status = response.status();
    let text = response.text().await.map_err(|e| {
        error!("{}: Failed to read response body: {}", state.player_name, e);
        AppError(e.to_string())
    })?;

    info!("{}: Join response status={}, body={}", state.player_name, status, text);

    let resp: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        error!("{}: Failed to parse JSON: {}", state.player_name, e);
        AppError(format!("Invalid JSON response: {}", e))
    })?;

    // Check for error in response
    if let Some(error_val) = resp.get("error") {
        let error_msg = error_val.as_str().unwrap_or("Unknown error");
        error!("{}: Oracle returned error: {}", state.player_name, error_msg);
        return Err(AppError(error_msg.to_string()));
    }

    let oracle_pubkey = hex::decode(resp["oracle_pubkey"].as_str().unwrap_or(""))
        .ok()
        .and_then(|b| secp256k1::PublicKey::from_slice(&b).ok());

    let commitment_point = hex::decode(resp["commitment_point"].as_str().unwrap_or(""))
        .ok()
        .and_then(|b| secp256k1::PublicKey::from_slice(&b).ok());

    let amount_shannons = resp["amount_shannons"].as_u64().unwrap_or(0);

    // Parse game_type from Oracle response
    let game_type: GameType = serde_json::from_value(resp["game_type"].clone())
        .unwrap_or(GameType::RockPaperScissors);

    let preimage = Preimage::random();
    let payment_hash = preimage.payment_hash();
    let salt = Salt::random();

    // =========================================================================
    // Payment hash setup: B submits its hash, gets A's hash
    // Invoice creation is handled by the frontend via direct Fiber RPC
    // =========================================================================

    // 1. Submit MY (B's) payment_hash to Oracle (so A can get it to create their invoice)
    let submit_hash_url = format!("{}/game/{}/payment-hash", state.oracle_url, req.game_id);
    let submit_hash_body = serde_json::json!({
        "player": Player::B,
        "payment_hash": payment_hash,
        "preimage": preimage,
    });

    state.http_client
        .post(&submit_hash_url)
        .json(&submit_hash_body)
        .send()
        .await
        .map_err(|e| AppError(format!("Failed to submit payment hash: {}", e)))?;

    info!("{}: Submitted payment_hash to Oracle for game {:?}", state.player_name, req.game_id);

    // 2. Get opponent's (A's) payment_hash from Oracle
    let get_hash_url = format!("{}/game/{}/payment-hash/A", state.oracle_url, req.game_id);
    let opponent_hash_resp = state.http_client
        .get(&get_hash_url)
        .send()
        .await
        .map_err(|e| AppError(format!("Failed to get opponent payment hash: {}", e)))?;

    if !opponent_hash_resp.status().is_success() {
        return Err(AppError("Opponent (A) hasn't submitted their payment hash. This shouldn't happen.".to_string()));
    }

    let opponent_hash_data: serde_json::Value = opponent_hash_resp
        .json()
        .await
        .map_err(|e| AppError(format!("Failed to parse opponent payment hash: {}", e)))?;

    let opponent_payment_hash_array = opponent_hash_data["payment_hash"]
        .as_array()
        .ok_or_else(|| AppError("Invalid opponent payment hash format: expected array".to_string()))?;

    let opponent_payment_hash_bytes: Vec<u8> = opponent_payment_hash_array
        .iter()
        .map(|v| v.as_u64().unwrap_or(0) as u8)
        .collect();

    let opponent_payment_hash = PaymentHash::from_bytes(
        opponent_payment_hash_bytes.as_slice().try_into()
            .map_err(|_| AppError("Invalid payment hash length".to_string()))?
    );

    info!("{}: Got opponent's payment_hash for game {:?}", state.player_name, req.game_id);

    // Note: Invoice creation and payment are now handled by the frontend
    // The frontend will:
    // 1. Create a hold invoice on its Fiber node using opponent's payment_hash
    // 2. Submit the invoice string to Oracle via POST /game/{id}/invoice
    // 3. Report back via POST /api/game/{id}/invoice-created
    // 4. Get opponent's invoice from Oracle and pay via Fiber RPC
    // 5. Report back via POST /api/game/{id}/payment-done

    // Save game state
    let game_state = PlayerGameState {
        role: Player::B,
        game_type,
        amount_shannons,
        preimage,
        payment_hash,
        opponent_payment_hash: Some(opponent_payment_hash),
        opponent_preimage: None,
        salt,
        action: None,
        oracle_pubkey,
        commitment_point,
        opponent_encrypted_preimage: None,
        my_commitment: None,
        opponent_commitment: None,
        opponent_action: None,
        phase: PlayerGamePhase::WaitingForAction,
        result: None,
        my_invoice_string: None,
        opponent_invoice_string: None,
        paid_opponent: false,
        oracle_secret_number: None,
    };

    state.games.write().unwrap().insert(req.game_id, game_state);

    info!("{}: Joined game {:?}", state.player_name, req.game_id);

    Ok(Json(JoinGameResponse {
        status: "joined".to_string(),
    }))
}

async fn play(
    State(state): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<PlayRequest>,
) -> Result<Json<PlayResponse>, AppError> {
    // =========================================================================
    // Game flow: commit + reveal
    //
    // Invoice creation and payment are handled entirely by the frontend
    // via direct Fiber RPC calls. The backend only manages game state.
    // =========================================================================
    let (role, action, salt, commitment) = {
        let mut games = state.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.action = Some(req.action.clone());

        let commitment = Commitment::new(&req.action.to_bytes(), &game.salt);
        game.my_commitment = Some(commitment);

        (game.role, req.action.clone(), game.salt.clone(), commitment)
    };

    // Submit commitment to Oracle
    let commit_url = format!("{}/game/{}/commit", state.oracle_url, game_id);
    let commit_body = serde_json::json!({
        "player": role,
        "commitment": commitment,
    });

    state
        .http_client
        .post(&commit_url)
        .json(&commit_body)
        .send()
        .await
        .map_err(|e| AppError(e.to_string()))?;

    info!("{}: Submitted commitment for game {:?}", state.player_name, game_id);

    {
        let mut games = state.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase = PlayerGamePhase::Committed;
    }

    // Submit reveal to Oracle
    let reveal_url = format!("{}/game/{}/reveal", state.oracle_url, game_id);
    let (commit_a, commit_b) = match role {
        Player::A => (commitment, commitment),
        Player::B => (commitment, commitment),
    };

    let reveal_body = serde_json::json!({
        "player": role,
        "action": action,
        "salt": salt,
        "commit_a": commit_a,
        "commit_b": commit_b,
    });

    let reveal_resp = state
        .http_client
        .post(&reveal_url)
        .json(&reveal_body)
        .send()
        .await
        .map_err(|e| AppError(e.to_string()))?;

    let reveal_result: serde_json::Value = reveal_resp
        .json()
        .await
        .map_err(|e| AppError(e.to_string()))?;

    info!("{}: Submitted reveal for game {:?}: {:?}", state.player_name, game_id, reveal_result);

    let status = reveal_result["status"].as_str().unwrap_or("unknown");
    {
        let mut games = state.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        if status == "game_complete" {
            game.phase = PlayerGamePhase::WaitingForResult;
        } else {
            game.phase = PlayerGamePhase::Revealed;
        }
    }

    Ok(Json(PlayResponse {
        status: status.to_string(),
    }))
}

async fn get_game_status(
    State(state): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<GameStatusResponse>, AppError> {
    // Check current phase
    let current_phase = {
        let games = state.games.read().unwrap();
        let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase
    };

    // If waiting for opponent, check if opponent has joined
    // When opponent joins, fetch their payment_hash and transition to WaitingForAction
    // (Frontend will handle invoice creation via direct Fiber RPC)
    if current_phase == PlayerGamePhase::WaitingForOpponent {
        let url = format!("{}/game/{}/status", state.oracle_url, game_id);
        if let Ok(resp) = state.http_client.get(&url).send().await {
            if let Ok(status_data) = resp.json::<serde_json::Value>().await {
                if status_data["has_opponent"].as_bool() == Some(true) {
                    // Opponent has joined! Get their payment_hash
                    let needs_hash = {
                        let games = state.games.read().unwrap();
                        games.get(&game_id).map(|g| g.opponent_payment_hash.is_none()).unwrap_or(false)
                    };

                    let mut hash_obtained = !needs_hash;

                    if needs_hash {
                        let get_hash_url = format!("{}/game/{}/payment-hash/B", state.oracle_url, game_id);
                        info!("{}: Trying to get B's payment_hash from {}", state.player_name, get_hash_url);

                        if let Ok(hash_resp) = state.http_client.get(&get_hash_url).send().await {
                            if hash_resp.status().is_success() {
                                if let Ok(hash_data) = hash_resp.json::<serde_json::Value>().await {
                                    if let Some(hash_array) = hash_data["payment_hash"].as_array() {
                                        let hash_bytes: Vec<u8> = hash_array
                                            .iter()
                                            .map(|v| v.as_u64().unwrap_or(0) as u8)
                                            .collect();

                                        if let Ok(hash_arr) = <[u8; 32]>::try_from(hash_bytes.as_slice()) {
                                            let opponent_payment_hash = PaymentHash::from_bytes(hash_arr);

                                            let mut games = state.games.write().unwrap();
                                            if let Some(game) = games.get_mut(&game_id) {
                                                game.opponent_payment_hash = Some(opponent_payment_hash);
                                            }

                                            hash_obtained = true;
                                            info!("{}: Got B's payment_hash for game {:?}", state.player_name, game_id);
                                        }
                                    }
                                }
                            } else {
                                info!("{}: B's payment_hash not available yet", state.player_name);
                            }
                        }
                    }

                    // Transition to WaitingForAction — frontend will create invoice
                    if hash_obtained {
                        let mut games = state.games.write().unwrap();
                        if let Some(game) = games.get_mut(&game_id) {
                            game.phase = PlayerGamePhase::WaitingForAction;
                        }
                    }
                }
            }
        }
    }

    // Check if we need to poll Oracle for result
    let should_poll = {
        let games = state.games.read().unwrap();
        let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;
        game.result.is_none() && (game.phase == PlayerGamePhase::Revealed || game.phase == PlayerGamePhase::WaitingForResult)
    };

    if should_poll {
        let url = format!("{}/game/{}/result", state.oracle_url, game_id);
        let resp = state
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError(e.to_string()))?;

        let result_data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError(e.to_string()))?;

        if result_data["status"].as_str() == Some("completed") {
            let mut games = state.games.write().unwrap();
            let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

            if let Some(result_str) = result_data["result"].as_str() {
                game.result = match result_str {
                    "AWins" => Some(GameResult::AWins),
                    "BWins" => Some(GameResult::BWins),
                    "Draw" => Some(GameResult::Draw),
                    _ => None,
                };
            }

            if let Some(game_data) = result_data.get("game_data") {
                let opp_action_key = match game.role {
                    Player::A => "action_b",
                    Player::B => "action_a",
                };

                if let Some(opp_action) = game_data.get(opp_action_key) {
                    game.opponent_action = serde_json::from_value(opp_action.clone()).ok();
                }

                // Extract oracle's secret number for Guess Number games
                if let Some(oracle_secret) = game_data.get("oracle_secret") {
                    if let Some(secret_num) = oracle_secret.get("secret_number").and_then(|v| v.as_u64()) {
                        game.oracle_secret_number = Some(secret_num as u8);
                    }
                }
            }

            // Extract opponent's preimage if we won (Oracle returns it)
            let preimage_key = match game.role {
                Player::A => "preimage_for_a",
                Player::B => "preimage_for_b",
            };
            if let Some(preimage_data) = result_data.get(preimage_key) {
                // Preimage is serialized as an array of bytes
                if let Some(preimage_array) = preimage_data.as_array() {
                    let preimage_bytes: Vec<u8> = preimage_array
                        .iter()
                        .map(|v| v.as_u64().unwrap_or(0) as u8)
                        .collect();
                    if preimage_bytes.len() == 32 {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&preimage_bytes);
                        game.opponent_preimage = Some(Preimage::from_bytes(arr));
                        info!("{}: Got opponent's preimage from Oracle for game {:?}", state.player_name, game_id);
                    }
                }
            }

            game.phase = PlayerGamePhase::WaitingForResult;
        }
    }

    let games = state.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    // Winner, loser, and draw can all settle
    // Winner: settle_invoice (claim funds) on frontend
    // Loser: cancel_invoice (release held funds) on frontend
    // Draw: cancel_invoice on frontend
    let can_settle = if game.phase == PlayerGamePhase::Settled {
        false
    } else {
        game.result.is_some()
    };

    // Provide hex-encoded hashes/preimage for frontend Fiber RPC calls
    let opponent_payment_hash_hex = game.opponent_payment_hash.as_ref().map(|h| {
        format!("0x{}", hex::encode(h.as_bytes()))
    });
    let opponent_preimage_hex = game.opponent_preimage.as_ref().map(|p| {
        format!("0x{}", hex::encode(p.as_bytes()))
    });
    let my_payment_hash_hex = Some(format!("0x{}", hex::encode(game.payment_hash.as_bytes())));

    Ok(Json(GameStatusResponse {
        role: game.role,
        phase: game.phase,
        result: game.result,
        my_action: game.action.clone(),
        opponent_action: game.opponent_action.clone(),
        can_settle,
        opponent_payment_hash: opponent_payment_hash_hex,
        opponent_preimage: opponent_preimage_hex,
        my_payment_hash: my_payment_hash_hex,
        oracle_secret_number: game.oracle_secret_number,
    }))
}

async fn settle(
    State(state): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<SettleResponse>, AppError> {
    // Get game state
    let (result, amount_won, role) = {
        let games = state.games.read().unwrap();
        let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

        let result = game.result.ok_or(AppError::from("Game not complete"))?;

        if game.phase == PlayerGamePhase::Settled {
            return Err(AppError::from("Game already settled"));
        }

        let amount_won = match (result, game.role) {
            (GameResult::AWins, Player::A) | (GameResult::BWins, Player::B) => game.amount_shannons as i64,
            (GameResult::BWins, Player::A) | (GameResult::AWins, Player::B) => -(game.amount_shannons as i64),
            (GameResult::Draw, _) => 0,
        };

        (result, amount_won, game.role)
    };

    // Settlement logic (Hold Invoice security model):
    //
    // All Fiber RPC calls (settle_invoice, cancel_invoice) are now performed
    // by the frontend directly on the player's own Fiber node.
    //
    // The backend only tracks the game phase transition.
    //
    // Winner frontend: calls settle_invoice with opponent's preimage (from /status response)
    // Loser frontend: calls cancel_invoice to refund opponent
    // Draw frontend: both call cancel_invoice

    info!("{}: Player {:?} marking game {:?} as settled: amount_won = {}",
          state.player_name, role, game_id, amount_won);

    {
        let mut games = state.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase = PlayerGamePhase::Settled;
    }

    Ok(Json(SettleResponse { result, amount_won }))
}

// ============================================================================
// Frontend-to-Backend notification handlers
// ============================================================================

/// Frontend reports that it created an invoice on its Fiber node and submitted to Oracle
async fn player_invoice_created(
    State(state): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<InvoiceCreatedRequest>,
) -> Result<Json<InvoiceCreatedResponse>, AppError> {
    let mut games = state.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    game.my_invoice_string = Some(req.invoice_string);

    info!("{}: Frontend reported invoice created for game {:?}", state.player_name, game_id);

    Ok(Json(InvoiceCreatedResponse {
        status: "ok".to_string(),
    }))
}

/// Frontend reports that it paid the opponent's invoice via Fiber RPC
async fn player_payment_done(
    State(state): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
    Json(_req): Json<PaymentDoneRequest>,
) -> Result<Json<PaymentDoneResponse>, AppError> {
    let mut games = state.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    game.paid_opponent = true;

    info!("{}: Frontend reported payment done for game {:?}", state.player_name, game_id);

    Ok(Json(PaymentDoneResponse {
        status: "ok".to_string(),
    }))
}

fn create_router(state: Arc<PlayerState>) -> Router {
    Router::new()
        .route("/api/player", get(get_player_info))
        .route("/api/games/available", get(get_available_games))
        .route("/api/games/mine", get(get_my_games))
        .route("/api/game/create", post(create_game))
        .route("/api/game/join", post(join_game))
        .route("/api/game/:game_id/play", post(play))
        .route("/api/game/:game_id/status", get(get_game_status))
        .route("/api/game/:game_id/settle", post(settle))
        .route("/api/game/:game_id/invoice-created", post(player_invoice_created))
        .route("/api/game/:game_id/payment-done", post(player_payment_done))
        .nest_service(
            "/",
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::CACHE_CONTROL,
                    http::HeaderValue::from_static("no-cache"),
                ))
                .service(ServeDir::new("static")),
        )
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let player_id = Uuid::new_v4();
    let player_name = std::env::var("PLAYER_NAME").unwrap_or_else(|_| "Player".to_string());
    let oracle_url = std::env::var("ORACLE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse()
        .unwrap_or(3001);

    // Fiber RPC URL is passed to frontend for direct browser-to-node calls
    let fiber_rpc_url = std::env::var("FIBER_RPC_URL").ok();

    if let Some(ref url) = fiber_rpc_url {
        info!("Fiber RPC URL: {} (frontend will call directly)", url);
    } else {
        info!("No FIBER_RPC_URL set (mock mode — no real Fiber payments)");
    }

    let state = Arc::new(PlayerState::new(player_id, player_name.clone(), oracle_url, fiber_rpc_url));

    info!("Player '{}' ID: {}", player_name, player_id);

    let app = create_router(state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    info!("Player service listening on http://0.0.0.0:{}", port);
    info!("  All Fiber RPC calls are made by the frontend directly");

    axum::serve(listener, app).await.unwrap();
}
