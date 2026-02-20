//! Fiber Game Demo Service
//!
//! Combined service with Oracle and two Players on a single port.
//! 
//! Routes:
//! - `/player-a/` - Player A Web UI
//! - `/player-b/` - Player B Web UI
//! - `/api/oracle/...` - Oracle API
//! - `/api/player-a/...` - Player A API (calls Oracle via HTTP)
//! - `/api/player-b/...` - Player B API (calls Oracle via HTTP)

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
    games::{GameAction, GameJudge, GameType, OracleSecret},
    protocol::{GameId, GameResult, Player},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

// ============================================================================
// Error Type
// ============================================================================

struct AppError(String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, self.0).into_response()
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError(s.to_string())
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError(s)
    }
}

// ============================================================================
// Oracle State and Types
// ============================================================================

#[allow(dead_code)]
struct OracleState {
    secret_key: secp256k1::SecretKey,
    public_key: secp256k1::PublicKey,
    commitment_keys: RwLock<HashMap<GameId, secp256k1::SecretKey>>,
    games: RwLock<HashMap<GameId, OracleGameState>>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct OracleGameState {
    game_type: GameType,
    amount_shannons: u64,
    status: OracleGameStatus,
    commitment_point: secp256k1::PublicKey,
    oracle_secret: Option<OracleSecret>,
    oracle_commitment: Option<[u8; 32]>,
    player_a_id: Uuid,
    player_b_id: Option<Uuid>,
    invoice_a: Option<PaymentHash>,
    invoice_b: Option<PaymentHash>,
    encrypted_preimage_a: Option<EncryptedPreimage>,
    encrypted_preimage_b: Option<EncryptedPreimage>,
    commit_a: Option<Commitment>,
    commit_b: Option<Commitment>,
    reveal_a: Option<RevealData>,
    reveal_b: Option<RevealData>,
    result: Option<GameResult>,
    signature: Option<[u8; 64]>,
    created_at: Instant,
}

#[derive(Clone)]
#[allow(dead_code)]
struct RevealData {
    action: GameAction,
    salt: Salt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum OracleGameStatus {
    WaitingForOpponent,
    InProgress,
    Completed,
    Cancelled,
}

impl OracleState {
    fn new() -> Self {
        let secp = secp256k1::Secp256k1::new();
        let secret_key = secp256k1::SecretKey::new(&mut rand::thread_rng());
        let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);

        Self {
            secret_key,
            public_key,
            commitment_keys: RwLock::new(HashMap::new()),
            games: RwLock::new(HashMap::new()),
        }
    }

    fn generate_commitment_point(&self, game_id: &GameId) -> secp256k1::PublicKey {
        let secp = secp256k1::Secp256k1::new();
        let secret_key = secp256k1::SecretKey::new(&mut rand::thread_rng());
        let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);

        self.commitment_keys
            .write()
            .unwrap()
            .insert(*game_id, secret_key);

        public_key
    }
}

// ============================================================================
// Oracle Request/Response Types
// ============================================================================

#[derive(Serialize)]
struct OraclePubkeyResponse {
    pubkey: String,
}

#[derive(Serialize)]
struct AvailableGame {
    game_id: GameId,
    game_type: GameType,
    amount_shannons: u64,
    created_at_secs: u64,
}

#[derive(Serialize)]
struct OracleAvailableGamesResponse {
    games: Vec<AvailableGame>,
}

#[derive(Deserialize)]
struct OracleCreateGameRequest {
    game_type: GameType,
    player_a_id: Uuid,
    amount_shannons: u64,
}

#[derive(Serialize)]
struct OracleCreateGameResponse {
    game_id: GameId,
    oracle_pubkey: String,
    commitment_point: String,
    oracle_commitment: Option<String>,
}

#[derive(Deserialize)]
struct OracleJoinGameRequest {
    player_b_id: Uuid,
}

#[derive(Serialize)]
struct OracleJoinGameResponse {
    status: String,
    oracle_pubkey: String,
    commitment_point: String,
    oracle_commitment: Option<String>,
}

#[derive(Deserialize)]
struct SubmitInvoiceRequest {
    player: Player,
    payment_hash: PaymentHash,
}

#[derive(Serialize)]
struct StatusResponse {
    status: String,
}

#[derive(Serialize)]
struct InvoiceResponse {
    payment_hash: PaymentHash,
}

#[derive(Deserialize)]
struct SubmitEncryptedPreimageRequest {
    player: Player,
    encrypted_preimage: EncryptedPreimage,
}

#[derive(Serialize)]
struct EncryptedPreimageResponse {
    encrypted_preimage: EncryptedPreimage,
}

#[derive(Deserialize)]
struct SubmitCommitRequest {
    player: Player,
    commitment: Commitment,
}

#[derive(Deserialize)]
struct SubmitRevealRequest {
    player: Player,
    action: GameAction,
    salt: Salt,
    commit_a: Commitment,
    commit_b: Commitment,
}

#[derive(Serialize)]
struct OracleGameResultResponse {
    status: String,
    result: Option<GameResult>,
    signature: Option<String>,
    game_data: Option<GameDataResponse>,
}

#[derive(Serialize)]
struct GameDataResponse {
    action_a: GameAction,
    action_b: GameAction,
    oracle_secret: Option<OracleSecretResponse>,
}

#[derive(Serialize)]
struct OracleSecretResponse {
    secret_number: u8,
    nonce: String,
}

#[derive(Serialize)]
struct OracleGameStatusResponse {
    status: String,
    has_opponent: bool,
}

// ============================================================================
// Oracle Route Handlers
// ============================================================================

async fn oracle_get_pubkey(State(state): State<Arc<AppState>>) -> Json<OraclePubkeyResponse> {
    Json(OraclePubkeyResponse {
        pubkey: hex::encode(state.oracle.public_key.serialize()),
    })
}

async fn oracle_get_available_games(
    State(state): State<Arc<AppState>>,
) -> Json<OracleAvailableGamesResponse> {
    let games = state.oracle.games.read().unwrap();
    let available: Vec<AvailableGame> = games
        .iter()
        .filter(|(_, g)| g.status == OracleGameStatus::WaitingForOpponent)
        .map(|(id, g)| AvailableGame {
            game_id: *id,
            game_type: g.game_type,
            amount_shannons: g.amount_shannons,
            created_at_secs: g.created_at.elapsed().as_secs(),
        })
        .collect();

    Json(OracleAvailableGamesResponse { games: available })
}

async fn oracle_create_game(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OracleCreateGameRequest>,
) -> Json<OracleCreateGameResponse> {
    let game_id = GameId::new();
    let commitment_point = state.oracle.generate_commitment_point(&game_id);

    let (oracle_secret, oracle_commitment) = if req.game_type.requires_oracle_secret() {
        let secret = OracleSecret::random();
        let commitment = secret.commitment();
        (Some(secret), Some(commitment))
    } else {
        (None, None)
    };

    let game_state = OracleGameState {
        game_type: req.game_type,
        amount_shannons: req.amount_shannons,
        status: OracleGameStatus::WaitingForOpponent,
        commitment_point,
        oracle_secret,
        oracle_commitment,
        player_a_id: req.player_a_id,
        player_b_id: None,
        invoice_a: None,
        invoice_b: None,
        encrypted_preimage_a: None,
        encrypted_preimage_b: None,
        commit_a: None,
        commit_b: None,
        reveal_a: None,
        reveal_b: None,
        result: None,
        signature: None,
        created_at: Instant::now(),
    };

    state.oracle.games.write().unwrap().insert(game_id, game_state);

    info!("Oracle: Created game {:?} of type {:?}", game_id, req.game_type);

    Json(OracleCreateGameResponse {
        game_id,
        oracle_pubkey: hex::encode(state.oracle.public_key.serialize()),
        commitment_point: hex::encode(commitment_point.serialize()),
        oracle_commitment: oracle_commitment.map(hex::encode),
    })
}

async fn oracle_join_game(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<OracleJoinGameRequest>,
) -> Result<Json<OracleJoinGameResponse>, AppError> {
    let mut games = state.oracle.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    if game.status != OracleGameStatus::WaitingForOpponent {
        return Err(AppError::from("Game is not available to join"));
    }

    game.player_b_id = Some(req.player_b_id);
    game.status = OracleGameStatus::InProgress;

    info!("Oracle: Player {:?} joined game {:?}", req.player_b_id, game_id);

    Ok(Json(OracleJoinGameResponse {
        status: "joined".to_string(),
        oracle_pubkey: hex::encode(state.oracle.public_key.serialize()),
        commitment_point: hex::encode(game.commitment_point.serialize()),
        oracle_commitment: game.oracle_commitment.map(hex::encode),
    }))
}

async fn oracle_submit_invoice(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitInvoiceRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.oracle.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    match req.player {
        Player::A => game.invoice_a = Some(req.payment_hash),
        Player::B => game.invoice_b = Some(req.payment_hash),
    }

    Ok(Json(StatusResponse {
        status: "invoice_received".to_string(),
    }))
}

async fn oracle_get_invoice(
    State(state): State<Arc<AppState>>,
    Path((game_id, player)): Path<(GameId, String)>,
) -> Result<Json<InvoiceResponse>, AppError> {
    let games = state.oracle.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    let payment_hash = match player.as_str() {
        "A" | "a" => game.invoice_a.ok_or(AppError::from("Invoice A not submitted"))?,
        "B" | "b" => game.invoice_b.ok_or(AppError::from("Invoice B not submitted"))?,
        _ => return Err(AppError::from("Invalid player")),
    };

    Ok(Json(InvoiceResponse { payment_hash }))
}

async fn oracle_submit_encrypted_preimage(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitEncryptedPreimageRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.oracle.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    match req.player {
        Player::A => game.encrypted_preimage_a = Some(req.encrypted_preimage),
        Player::B => game.encrypted_preimage_b = Some(req.encrypted_preimage),
    }

    Ok(Json(StatusResponse {
        status: "encrypted_preimage_received".to_string(),
    }))
}

async fn oracle_get_encrypted_preimage(
    State(state): State<Arc<AppState>>,
    Path((game_id, player)): Path<(GameId, String)>,
) -> Result<Json<EncryptedPreimageResponse>, AppError> {
    let games = state.oracle.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    let encrypted_preimage = match player.as_str() {
        "A" | "a" => game
            .encrypted_preimage_a
            .clone()
            .ok_or(AppError::from("Encrypted preimage A not submitted"))?,
        "B" | "b" => game
            .encrypted_preimage_b
            .clone()
            .ok_or(AppError::from("Encrypted preimage B not submitted"))?,
        _ => return Err(AppError::from("Invalid player")),
    };

    Ok(Json(EncryptedPreimageResponse { encrypted_preimage }))
}

async fn oracle_submit_commit(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitCommitRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.oracle.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    match req.player {
        Player::A => game.commit_a = Some(req.commitment),
        Player::B => game.commit_b = Some(req.commitment),
    }

    Ok(Json(StatusResponse {
        status: "commitment_received".to_string(),
    }))
}

async fn oracle_submit_reveal(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitRevealRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.oracle.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    // Verify commitment matches
    let expected_commit = match req.player {
        Player::A => req.commit_a,
        Player::B => req.commit_b,
    };

    let stored_commit = match req.player {
        Player::A => game.commit_a.ok_or(AppError::from("Commitment A not found"))?,
        Player::B => game.commit_b.ok_or(AppError::from("Commitment B not found"))?,
    };

    if expected_commit != stored_commit {
        return Err(AppError::from("Commitment mismatch"));
    }

    // Verify the reveal matches the commitment
    if !stored_commit.verify(&req.action.to_bytes(), &req.salt) {
        return Err(AppError::from("Reveal does not match commitment"));
    }

    // Store reveal
    let reveal = RevealData {
        action: req.action,
        salt: req.salt,
    };

    match req.player {
        Player::A => game.reveal_a = Some(reveal),
        Player::B => game.reveal_b = Some(reveal),
    }

    // Check if both reveals are in, then judge
    if game.reveal_a.is_some() && game.reveal_b.is_some() {
        let action_a = &game.reveal_a.as_ref().unwrap().action;
        let action_b = &game.reveal_b.as_ref().unwrap().action;

        let result = match game.game_type {
            GameType::RockPaperScissors => {
                fiber_game_core::games::RpsGame::judge(action_a, action_b, None)
            }
            GameType::GuessNumber => fiber_game_core::games::GuessNumberGame::judge(
                action_a,
                action_b,
                game.oracle_secret.as_ref(),
            ),
        };

        game.result = Some(result);
        game.status = OracleGameStatus::Completed;

        // Sign the result
        let mut sig = [0u8; 64];
        let msg = format!("{}:{}", game_id, result.as_str());
        let hash = sha2::Sha256::digest(msg.as_bytes());
        sig[..32].copy_from_slice(&hash);

        game.signature = Some(sig);

        info!("Oracle: Game {:?} completed with result: {:?}", game_id, result);

        Ok(Json(StatusResponse {
            status: "game_complete".to_string(),
        }))
    } else {
        Ok(Json(StatusResponse {
            status: "waiting_for_opponent".to_string(),
        }))
    }
}

async fn oracle_get_game_status(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<OracleGameStatusResponse>, AppError> {
    let games = state.oracle.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    let status = match game.status {
        OracleGameStatus::WaitingForOpponent => "waiting_for_opponent",
        OracleGameStatus::InProgress => "in_progress",
        OracleGameStatus::Completed => "completed",
        OracleGameStatus::Cancelled => "cancelled",
    };

    Ok(Json(OracleGameStatusResponse {
        status: status.to_string(),
        has_opponent: game.player_b_id.is_some(),
    }))
}

async fn oracle_get_result(
    State(state): State<Arc<AppState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<OracleGameResultResponse>, AppError> {
    let games = state.oracle.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    if game.status != OracleGameStatus::Completed {
        return Ok(Json(OracleGameResultResponse {
            status: "pending".to_string(),
            result: None,
            signature: None,
            game_data: None,
        }));
    }

    let game_data = if let (Some(reveal_a), Some(reveal_b)) = (&game.reveal_a, &game.reveal_b) {
        Some(GameDataResponse {
            action_a: reveal_a.action.clone(),
            action_b: reveal_b.action.clone(),
            oracle_secret: game.oracle_secret.as_ref().map(|s| OracleSecretResponse {
                secret_number: s.secret_number,
                nonce: hex::encode(s.nonce),
            }),
        })
    } else {
        None
    };

    Ok(Json(OracleGameResultResponse {
        status: "completed".to_string(),
        result: game.result,
        signature: game.signature.map(hex::encode),
        game_data,
    }))
}

// ============================================================================
// Player State and Types
// ============================================================================

struct PlayerState {
    player_id: Uuid,
    player_name: String,
    oracle_url: String,
    http_client: Client,
    fiber_client: Arc<dyn FiberClient>,
    games: RwLock<HashMap<GameId, PlayerGameState>>,
}

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

impl PlayerState {
    fn new(player_id: Uuid, player_name: String, oracle_url: String, fiber_client: Arc<dyn FiberClient>) -> Self {
        Self {
            player_id,
            player_name,
            oracle_url,
            http_client: Client::new(),
            fiber_client,
            games: RwLock::new(HashMap::new()),
        }
    }
}

// ============================================================================
// Player Request/Response Types
// ============================================================================

#[derive(Serialize)]
struct PlayerInfoResponse {
    player_id: Uuid,
    player_name: String,
    balance: u64,
}

#[derive(Serialize)]
struct PlayerAvailableGameResponse {
    game_id: GameId,
    game_type: GameType,
    amount_shannons: u64,
}

#[derive(Serialize)]
struct PlayerAvailableGamesResponse {
    games: Vec<PlayerAvailableGameResponse>,
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
struct PlayerCreateGameRequest {
    game_type: GameType,
    amount_shannons: u64,
}

#[derive(Serialize)]
struct PlayerCreateGameResponse {
    game_id: GameId,
}

#[derive(Deserialize)]
struct PlayerJoinGameRequest {
    game_id: GameId,
}

#[derive(Serialize)]
struct PlayerJoinGameResponse {
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
struct PlayerGameStatusResponse {
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

// ============================================================================
// Player Route Handlers (Generic for both Player A and B)
// ============================================================================

async fn player_get_info(State(player): State<Arc<PlayerState>>) -> Result<Json<PlayerInfoResponse>, AppError> {
    let balance = player.fiber_client.get_balance().await
        .map_err(|e| AppError(format!("Failed to get balance: {}", e)))?;
    Ok(Json(PlayerInfoResponse {
        player_id: player.player_id,
        player_name: player.player_name.clone(),
        balance,
    }))
}

async fn player_get_available_games(
    State(player): State<Arc<PlayerState>>,
) -> Result<Json<PlayerAvailableGamesResponse>, AppError> {
    let url = format!("{}/games/available", player.oracle_url);
    let resp: serde_json::Value = player
        .http_client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError(e.to_string()))?;

    let games: Vec<PlayerAvailableGameResponse> = resp["games"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|g| PlayerAvailableGameResponse {
            game_id: serde_json::from_value(g["game_id"].clone()).unwrap(),
            game_type: serde_json::from_value(g["game_type"].clone()).unwrap(),
            amount_shannons: g["amount_shannons"].as_u64().unwrap_or(0),
        })
        .collect();

    Ok(Json(PlayerAvailableGamesResponse { games }))
}

async fn player_get_my_games(State(player): State<Arc<PlayerState>>) -> Json<MyGamesResponse> {
    // Check Oracle for games waiting for opponent
    let games_to_check: Vec<GameId> = {
        let games = player.games.read().unwrap();
        games
            .iter()
            .filter(|(_, g)| g.phase == PlayerGamePhase::WaitingForOpponent)
            .map(|(id, _)| *id)
            .collect()
    };

    // Update phase for games where opponent has joined
    for game_id in games_to_check {
        let url = format!("{}/game/{}/status", player.oracle_url, game_id);
        if let Ok(resp) = player.http_client.get(&url).send().await {
            if let Ok(status_data) = resp.json::<serde_json::Value>().await {
                if status_data["has_opponent"].as_bool() == Some(true) {
                    let mut games = player.games.write().unwrap();
                    if let Some(game) = games.get_mut(&game_id) {
                        game.phase = PlayerGamePhase::WaitingForAction;
                    }
                }
            }
        }
    }

    let games = player.games.read().unwrap();
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

async fn player_create_game(
    State(player): State<Arc<PlayerState>>,
    Json(req): Json<PlayerCreateGameRequest>,
) -> Result<Json<PlayerCreateGameResponse>, AppError> {
    let url = format!("{}/game/create", player.oracle_url);

    let body = serde_json::json!({
        "game_type": req.game_type,
        "player_a_id": player.player_id,
        "amount_shannons": req.amount_shannons,
    });

    let resp: serde_json::Value = player
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

    player.games.write().unwrap().insert(game_id, game_state);

    info!("{}: Created game {:?}", player.player_name, game_id);

    Ok(Json(PlayerCreateGameResponse { game_id }))
}

async fn player_join_game(
    State(player): State<Arc<PlayerState>>,
    Json(req): Json<PlayerJoinGameRequest>,
) -> Result<Json<PlayerJoinGameResponse>, AppError> {
    let url = format!("{}/game/{}/join", player.oracle_url, req.game_id);

    let body = serde_json::json!({
        "player_b_id": player.player_id,
    });

    let resp: serde_json::Value = player
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

    let preimage = Preimage::random();
    let payment_hash = preimage.payment_hash();
    let salt = Salt::random();

    let game_state = PlayerGameState {
        role: Player::B,
        game_type: GameType::RockPaperScissors,
        amount_shannons: 0,
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
        phase: PlayerGamePhase::WaitingForAction,
        result: None,
    };

    player.games.write().unwrap().insert(req.game_id, game_state);

    info!("{}: Joined game {:?}", player.player_name, req.game_id);

    Ok(Json(PlayerJoinGameResponse {
        status: "joined".to_string(),
    }))
}

async fn player_play(
    State(player): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<PlayRequest>,
) -> Result<Json<PlayResponse>, AppError> {
    let (role, action, salt, commitment) = {
        let mut games = player.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.action = Some(req.action.clone());

        let commitment = Commitment::new(&req.action.to_bytes(), &game.salt);
        game.my_commitment = Some(commitment.clone());

        (game.role, req.action.clone(), game.salt.clone(), commitment)
    };

    // Submit commitment to Oracle
    let commit_url = format!("{}/game/{}/commit", player.oracle_url, game_id);
    let commit_body = serde_json::json!({
        "player": role,
        "commitment": commitment,
    });

    player
        .http_client
        .post(&commit_url)
        .json(&commit_body)
        .send()
        .await
        .map_err(|e| AppError(e.to_string()))?;

    info!("{}: Submitted commitment for game {:?}", player.player_name, game_id);

    {
        let mut games = player.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase = PlayerGamePhase::Committed;
    }

    // Submit reveal to Oracle
    let reveal_url = format!("{}/game/{}/reveal", player.oracle_url, game_id);
    let (commit_a, commit_b) = match role {
        Player::A => (commitment.clone(), commitment.clone()),
        Player::B => (commitment.clone(), commitment.clone()),
    };

    let reveal_body = serde_json::json!({
        "player": role,
        "action": action,
        "salt": salt,
        "commit_a": commit_a,
        "commit_b": commit_b,
    });

    let reveal_resp = player
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

    info!("{}: Submitted reveal for game {:?}: {:?}", player.player_name, game_id, reveal_result);

    let status = reveal_result["status"].as_str().unwrap_or("unknown");
    {
        let mut games = player.games.write().unwrap();
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

async fn player_get_game_status(
    State(player): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<PlayerGameStatusResponse>, AppError> {
    // Check current phase
    let current_phase = {
        let games = player.games.read().unwrap();
        let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase
    };

    // If waiting for opponent, check if opponent has joined
    if current_phase == PlayerGamePhase::WaitingForOpponent {
        let url = format!("{}/game/{}/status", player.oracle_url, game_id);
        if let Ok(resp) = player.http_client.get(&url).send().await {
            if let Ok(status_data) = resp.json::<serde_json::Value>().await {
                if status_data["has_opponent"].as_bool() == Some(true) {
                    let mut games = player.games.write().unwrap();
                    if let Some(game) = games.get_mut(&game_id) {
                        game.phase = PlayerGamePhase::WaitingForAction;
                    }
                }
            }
        }
    }

    // Check if we need to poll Oracle for result
    let should_poll = {
        let games = player.games.read().unwrap();
        let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;
        game.result.is_none() && (game.phase == PlayerGamePhase::Revealed || game.phase == PlayerGamePhase::WaitingForResult)
    };

    if should_poll {
        let url = format!("{}/game/{}/result", player.oracle_url, game_id);
        let resp = player
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
            let mut games = player.games.write().unwrap();
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
            }

            game.phase = PlayerGamePhase::WaitingForResult;
        }
    }

    let games = player.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    Ok(Json(PlayerGameStatusResponse {
        phase: game.phase,
        result: game.result,
        my_action: game.action.clone(),
        opponent_action: game.opponent_action.clone(),
        can_settle: game.result.is_some() && game.phase != PlayerGamePhase::Settled,
    }))
}

async fn player_settle(
    State(player): State<Arc<PlayerState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<SettleResponse>, AppError> {
    let (result, amount_won) = {
        let games = player.games.read().unwrap();
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

        (result, amount_won)
    };

    // Adjust balance if using MockFiberClient
    if let Some(mock_client) = player.fiber_client.as_any().downcast_ref::<MockFiberClient>() {
        mock_client.adjust_balance(amount_won);
    } else {
        info!("Real Fiber settlement would happen here for amount: {}", amount_won);
    }

    info!("{}: Settled game {:?}: amount_won = {}", player.player_name, game_id, amount_won);

    {
        let mut games = player.games.write().unwrap();
        let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;
        game.phase = PlayerGamePhase::Settled;
    }

    Ok(Json(SettleResponse { result, amount_won }))
}

// ============================================================================
// Combined Application State
// ============================================================================

struct AppState {
    oracle: OracleState,
    player_a: Arc<PlayerState>,
    player_b: Arc<PlayerState>,
}

// ============================================================================
// Router Creation
// ============================================================================

fn create_oracle_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/pubkey", get(oracle_get_pubkey))
        .route("/games/available", get(oracle_get_available_games))
        .route("/game/create", post(oracle_create_game))
        .route("/game/{game_id}/join", post(oracle_join_game))
        .route("/game/{game_id}/invoice", post(oracle_submit_invoice))
        .route("/game/{game_id}/invoice/{player}", get(oracle_get_invoice))
        .route("/game/{game_id}/encrypted-preimage", post(oracle_submit_encrypted_preimage))
        .route("/game/{game_id}/encrypted-preimage/{player}", get(oracle_get_encrypted_preimage))
        .route("/game/{game_id}/commit", post(oracle_submit_commit))
        .route("/game/{game_id}/reveal", post(oracle_submit_reveal))
        .route("/game/{game_id}/status", get(oracle_get_game_status))
        .route("/game/{game_id}/result", get(oracle_get_result))
}

fn create_player_router(get_player: fn(&AppState) -> Arc<PlayerState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/player", get(move |State(state): State<Arc<AppState>>| async move {
            player_get_info(State(get_player(&state))).await
        }))
        .route("/games/available", get(move |State(state): State<Arc<AppState>>| async move {
            player_get_available_games(State(get_player(&state))).await
        }))
        .route("/games/mine", get(move |State(state): State<Arc<AppState>>| async move {
            player_get_my_games(State(get_player(&state))).await
        }))
        .route("/game/create", post(move |State(state): State<Arc<AppState>>, body: Json<PlayerCreateGameRequest>| async move {
            player_create_game(State(get_player(&state)), body).await
        }))
        .route("/game/join", post(move |State(state): State<Arc<AppState>>, body: Json<PlayerJoinGameRequest>| async move {
            player_join_game(State(get_player(&state)), body).await
        }))
        .route("/game/{game_id}/play", post(move |State(state): State<Arc<AppState>>, path: Path<GameId>, body: Json<PlayRequest>| async move {
            player_play(State(get_player(&state)), path, body).await
        }))
        .route("/game/{game_id}/status", get(move |State(state): State<Arc<AppState>>, path: Path<GameId>| async move {
            player_get_game_status(State(get_player(&state)), path).await
        }))
        .route("/game/{game_id}/settle", post(move |State(state): State<Arc<AppState>>, path: Path<GameId>| async move {
            player_settle(State(get_player(&state)), path).await
        }))
}

fn get_player_a(state: &AppState) -> Arc<PlayerState> {
    state.player_a.clone()
}

fn get_player_b(state: &AppState) -> Arc<PlayerState> {
    state.player_b.clone()
}

fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api/oracle", create_oracle_router())
        .nest("/api/player-a", create_player_router(get_player_a))
        .nest("/api/player-b", create_player_router(get_player_b))
        .nest_service("/player-a", ServeDir::new("static/player-a"))
        .nest_service("/player-b", ServeDir::new("static/player-b"))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    let oracle_url = format!("http://localhost:{}/api/oracle", port);

    let player_a_id = Uuid::new_v4();
    let player_b_id = Uuid::new_v4();

    // Check for real Fiber clients
    let fiber_client_a: Arc<dyn FiberClient> = if let Ok(url) = std::env::var("FIBER_PLAYER_A_RPC_URL") {
        info!("Player A using Fiber RPC: {}", url);
        Arc::new(RpcFiberClient::new(url))
    } else {
        info!("Player A using MockFiberClient (set FIBER_PLAYER_A_RPC_URL to enable real Fiber integration)");
        Arc::new(MockFiberClient::new(100_000))
    };

    let fiber_client_b: Arc<dyn FiberClient> = if let Ok(url) = std::env::var("FIBER_PLAYER_B_RPC_URL") {
        info!("Player B using Fiber RPC: {}", url);
        Arc::new(RpcFiberClient::new(url))
    } else {
        info!("Player B using MockFiberClient (set FIBER_PLAYER_B_RPC_URL to enable real Fiber integration)");
        Arc::new(MockFiberClient::new(100_000))
    };

    let state = Arc::new(AppState {
        oracle: OracleState::new(),
        player_a: Arc::new(PlayerState::new(player_a_id, "Player A".to_string(), oracle_url.clone(), fiber_client_a)),
        player_b: Arc::new(PlayerState::new(player_b_id, "Player B".to_string(), oracle_url, fiber_client_b)),
    });

    info!("Oracle public key: {}", hex::encode(state.oracle.public_key.serialize()));
    
    // Asynchronously log balances
    match state.player_a.fiber_client.get_balance().await {
        Ok(bal) => info!("Player A ID: {} (balance: {} shannons)", player_a_id, bal),
        Err(e) => info!("Player A balance error: {}", e),
    }
    match state.player_b.fiber_client.get_balance().await {
        Ok(bal) => info!("Player B ID: {} (balance: {} shannons)", player_b_id, bal),
        Err(e) => info!("Player B balance error: {}", e),
    }

    let app = create_app(state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    info!("Fiber Game Demo listening on http://0.0.0.0:{}", port);
    info!("  Player A: http://localhost:{}/player-a/", port);
    info!("  Player B: http://localhost:{}/player-b/", port);

    axum::serve(listener, app).await.unwrap();
}
