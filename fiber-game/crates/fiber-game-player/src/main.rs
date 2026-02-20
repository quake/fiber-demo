//! Fiber Game Player Service
//!
//! HTTP service with Web UI for players to create/join games and play.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use fiber_game_core::{
    crypto::{Commitment, EncryptedPreimage, PaymentHash, Preimage, Salt},
    fiber::{FiberClient, MockFiberClient, RpcFiberClient},
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
use tracing::{info, Level};
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
    oracle_url: String,
    http_client: Client,
    fiber_client: Arc<dyn FiberClient>,
    games: RwLock<HashMap<GameId, PlayerGameState>>,
}

/// State of a game from player's perspective
#[derive(Clone)]
#[allow(dead_code)]
struct PlayerGameState {
    role: Player,
    game_type: GameType,
    amount_shannons: u64,
    preimage: Preimage,
    payment_hash: PaymentHash,
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
    balance: u64,
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
    phase: PlayerGamePhase,
    result: Option<GameResult>,
    my_action: Option<GameAction>,
    opponent_action: Option<GameAction>,
    can_settle: bool,
}

#[derive(Serialize)]
struct SettleResponse {
    result: GameResult,
    amount_won: i64,
}

impl PlayerState {
    fn new(player_id: Uuid, oracle_url: String, fiber_client: Arc<dyn FiberClient>) -> Self {
        Self {
            player_id,
            oracle_url,
            http_client: Client::new(),
            fiber_client,
            games: RwLock::new(HashMap::new()),
        }
    }
}

// === Route handlers ===

async fn get_player_info(State(state): State<Arc<PlayerState>>) -> Result<Json<PlayerInfoResponse>, AppError> {
    let balance = state.fiber_client.get_balance().await
        .map_err(|e| AppError(format!("Failed to get balance: {}", e)))?;
    Ok(Json(PlayerInfoResponse {
        player_id: state.player_id,
        balance,
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

    let games: Vec<AvailableGameResponse> = resp["games"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|g| AvailableGameResponse {
            game_id: serde_json::from_value(g["game_id"].clone()).unwrap(),
            game_type: serde_json::from_value(g["game_type"].clone()).unwrap(),
            amount_shannons: g["amount_shannons"].as_u64().unwrap_or(0),
        })
        .collect();

    Ok(Json(AvailableGamesResponse { games }))
}

async fn get_my_games(State(state): State<Arc<PlayerState>>) -> Json<MyGamesResponse> {
    // First, check Oracle for any games waiting for opponent
    let games_to_check: Vec<GameId> = {
        let games = state.games.read().unwrap();
        games
            .iter()
            .filter(|(_, g)| g.phase == PlayerGamePhase::WaitingForOpponent)
            .map(|(id, _)| *id)
            .collect()
    };

    // Update phase for games where opponent has joined
    for game_id in games_to_check {
        let url = format!("{}/game/{}/status", state.oracle_url, game_id);
        if let Ok(resp) = state.http_client.get(&url).send().await {
            if let Ok(status_data) = resp.json::<serde_json::Value>().await {
                if status_data["has_opponent"].as_bool() == Some(true) {
                    let mut games = state.games.write().unwrap();
                    if let Some(game) = games.get_mut(&game_id) {
                        game.phase = PlayerGamePhase::WaitingForAction;
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

    // Generate preimage and salt
    let preimage = Preimage::random();
    let payment_hash = preimage.payment_hash();
    let salt = Salt::random();

    let game_state = PlayerGameState {
        role: Player::A,
        game_type: req.game_type,
        amount_shannons: req.amount_shannons,
        preimage,
        payment_hash,
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
    };

    state.games.write().unwrap().insert(game_id, game_state);

    info!("Created game {:?}", game_id);

    Ok(Json(CreateGameResponse { game_id }))
}

async fn join_game(
    State(state): State<Arc<PlayerState>>,
    Json(req): Json<JoinGameRequest>,
) -> Result<Json<JoinGameResponse>, AppError> {
    let url = format!("{}/game/{}/join", state.oracle_url, req.game_id);

    let body = serde_json::json!({
        "player_b_id": state.player_id,
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

    let oracle_pubkey = hex::decode(resp["oracle_pubkey"].as_str().unwrap_or(""))
        .ok()
        .and_then(|b| secp256k1::PublicKey::from_slice(&b).ok());

    let commitment_point = hex::decode(resp["commitment_point"].as_str().unwrap_or(""))
        .ok()
        .and_then(|b| secp256k1::PublicKey::from_slice(&b).ok());

    // Generate preimage and salt
    let preimage = Preimage::random();
    let payment_hash = preimage.payment_hash();
    let salt = Salt::random();

    let game_state = PlayerGameState {
        role: Player::B,
        game_type: GameType::RockPaperScissors, // Will be updated
        amount_shannons: 0,                           // Will be updated
        preimage,
        payment_hash,
        salt,
        action: None,
        oracle_pubkey,
        commitment_point,
        opponent_encrypted_preimage: None,
        my_commitment: None,
        opponent_commitment: None,
        opponent_action: None,
        phase: PlayerGamePhase::ExchangingInvoices,
        result: None,
    };

    state.games.write().unwrap().insert(req.game_id, game_state);

    info!("Joined game {:?}", req.game_id);

    Ok(Json(JoinGameResponse {
        status: "joined".to_string(),
    }))
}

#[axum::debug_handler]
async fn play(
    State(state): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<PlayRequest>,
) -> Result<Json<PlayResponse>, AppError> {
    // Update local state and create commitment
    let (role, action, salt, commitment) = {
        let mut games = state.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.action = Some(req.action.clone());

        // Create commitment
        let commitment = Commitment::new(&req.action.to_bytes(), &game.salt);
        game.my_commitment = Some(commitment.clone());
        
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

    info!("Submitted commitment for game {:?}", game_id);

    // Update phase to Committed
    {
        let mut games = state.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase = PlayerGamePhase::Committed;
    }

    // Now submit reveal to Oracle
    // The Oracle's reveal endpoint expects commit_a and commit_b
    // We use our commitment for our slot, and a dummy for the opponent
    // (Oracle will use what it has stored)
    let reveal_url = format!("{}/game/{}/reveal", state.oracle_url, game_id);
    let (commit_a, commit_b) = match role {
        Player::A => (commitment.clone(), commitment.clone()), // Oracle uses stored commit_b
        Player::B => (commitment.clone(), commitment.clone()), // Oracle uses stored commit_a
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

    info!("Submitted reveal for game {:?}: {:?}", game_id, reveal_result);

    // Update phase based on reveal response
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
    if current_phase == PlayerGamePhase::WaitingForOpponent {
        let url = format!("{}/game/{}/status", state.oracle_url, game_id);
        if let Ok(resp) = state.http_client.get(&url).send().await {
            if let Ok(status_data) = resp.json::<serde_json::Value>().await {
                if status_data["has_opponent"].as_bool() == Some(true) {
                    // Opponent has joined, update phase
                    let mut games = state.games.write().unwrap();
                    if let Some(game) = games.get_mut(&game_id) {
                        game.phase = PlayerGamePhase::WaitingForAction;
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
        // Poll Oracle for result
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

        // Check if game is completed
        if result_data["status"].as_str() == Some("completed") {
            let mut games = state.games.write().unwrap();
            let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
            
            // Parse result
            if let Some(result_str) = result_data["result"].as_str() {
                game.result = match result_str {
                    "AWins" => Some(GameResult::AWins),
                    "BWins" => Some(GameResult::BWins),
                    "Draw" => Some(GameResult::Draw),
                    _ => None,
                };
            }

            // Parse opponent action from game_data
            if let Some(game_data) = result_data.get("game_data") {
                let opp_action_key = match game.role {
                    Player::A => "action_b",
                    Player::B => "action_a",
                };
                
                if let Some(opp_action) = game_data.get(opp_action_key) {
                    game.opponent_action = serde_json::from_value(opp_action.clone()).ok();
                }
            }

            game.phase = PlayerGamePhase::WaitingForResult;
            if game.result.is_some() {
                game.phase = PlayerGamePhase::WaitingForResult;
            }
        }
    }

    // Return current state
    let games = state.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    Ok(Json(GameStatusResponse {
        phase: game.phase,
        result: game.result,
        my_action: game.action.clone(),
        opponent_action: game.opponent_action.clone(),
        can_settle: game.result.is_some() && game.phase != PlayerGamePhase::Settled,
    }))
}

async fn settle(
    State(state): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<SettleResponse>, AppError> {
    let (result, amount_won, already_settled) = {
        let games = state.games.read().unwrap();
        let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

        let result = game.result.ok_or(AppError::from("Game not complete"))?;

        // Check if already settled
        if game.phase == PlayerGamePhase::Settled {
            return Err(AppError::from("Game already settled"));
        }

        // Determine amount won/lost based on result and role
        let amount_won = match (result, game.role) {
            (GameResult::AWins, Player::A) | (GameResult::BWins, Player::B) => game.amount_shannons as i64,
            (GameResult::BWins, Player::A) | (GameResult::AWins, Player::B) => -(game.amount_shannons as i64),
            (GameResult::Draw, _) => 0,
        };

        (result, amount_won, game.phase == PlayerGamePhase::Settled)
    };

    if already_settled {
        return Err(AppError::from("Game already settled"));
    }

    // Adjust balance if using MockFiberClient
    // In a real implementation, this would involve actual Fiber invoice settlement
    // using the extracted preimage from the oracle signature.
    if let Some(mock_client) = state.fiber_client.as_any().downcast_ref::<MockFiberClient>() {
        mock_client.adjust_balance(amount_won);
    } else {
        info!("Real Fiber settlement would happen here for amount: {}", amount_won);
    }
    
    info!("Settled game {:?}: amount_won = {}", game_id, amount_won);

    // Update phase to Settled
    {
        let mut games = state.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase = PlayerGamePhase::Settled;
    }

    Ok(Json(SettleResponse { result, amount_won }))
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
        .nest_service("/", ServeDir::new("static"))
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
    let oracle_url = std::env::var("ORACLE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse()
        .unwrap_or(3001);

    // Initialize Fiber client (Mock or RPC)
    let fiber_client: Arc<dyn FiberClient> = if let Ok(url) = std::env::var("FIBER_RPC_URL") {
        info!("Using Fiber RPC: {}", url);
        Arc::new(RpcFiberClient::new(url))
    } else {
        info!("Using MockFiberClient (set FIBER_RPC_URL to enable real Fiber integration)");
        Arc::new(MockFiberClient::new(100_000)) // 100k shannons initial balance
    };

    let state = Arc::new(PlayerState::new(player_id, oracle_url, fiber_client));

    info!("Player ID: {}", player_id);
    match state.fiber_client.get_balance().await {
        Ok(balance) => info!("Balance: {} shannons", balance),
        Err(e) => info!("Balance: Error ({})", e),
    }

    let app = create_router(state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    info!("Player service listening on http://0.0.0.0:{}", port);

    axum::serve(listener, app).await.unwrap();
}
