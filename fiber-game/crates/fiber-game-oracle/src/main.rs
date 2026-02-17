//! Fiber Game Oracle Service
//!
//! HTTP service that manages game sessions, collects reveals, and signs results.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use fiber_game_core::{
    crypto::{Commitment, EncryptedPreimage, PaymentHash},
    games::{GameAction, GameJudge, GameType, OracleSecret},
    protocol::{GameId, GameResult, Player},
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
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

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError(s.to_string())
    }
}

/// Oracle state
#[allow(dead_code)]
struct OracleState {
    /// Oracle's secret key (for signing)
    secret_key: secp256k1::SecretKey,
    /// Oracle's public key
    public_key: secp256k1::PublicKey,
    /// Commitment keypair for each game
    commitment_keys: RwLock<HashMap<GameId, secp256k1::SecretKey>>,
    /// Active games
    games: RwLock<HashMap<GameId, GameState>>,
}

/// State of a game session
#[derive(Clone)]
#[allow(dead_code)]
struct GameState {
    game_type: GameType,
    amount_sat: u64,
    status: GameStatus,
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
    salt: fiber_game_core::crypto::Salt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum GameStatus {
    WaitingForOpponent,
    InProgress,
    Completed,
    Cancelled,
}

// === Request/Response types ===

#[derive(Serialize)]
struct OraclePubkeyResponse {
    pubkey: String,
}

#[derive(Serialize)]
struct AvailableGame {
    game_id: GameId,
    game_type: GameType,
    amount_sat: u64,
    created_at_secs: u64,
}

#[derive(Serialize)]
struct AvailableGamesResponse {
    games: Vec<AvailableGame>,
}

#[derive(Deserialize)]
struct CreateGameRequest {
    game_type: GameType,
    player_a_id: Uuid,
    amount_sat: u64,
}

#[derive(Serialize)]
struct CreateGameResponse {
    game_id: GameId,
    oracle_pubkey: String,
    commitment_point: String,
    oracle_commitment: Option<String>,
}

#[derive(Deserialize)]
struct JoinGameRequest {
    player_b_id: Uuid,
}

#[derive(Serialize)]
struct JoinGameResponse {
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
    salt: fiber_game_core::crypto::Salt,
    commit_a: Commitment,
    commit_b: Commitment,
}

#[derive(Serialize)]
struct GameResultResponse {
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

// === Route handlers ===

async fn get_pubkey(State(state): State<Arc<OracleState>>) -> Json<OraclePubkeyResponse> {
    Json(OraclePubkeyResponse {
        pubkey: hex::encode(state.public_key.serialize()),
    })
}

async fn get_available_games(
    State(state): State<Arc<OracleState>>,
) -> Json<AvailableGamesResponse> {
    let games = state.games.read().unwrap();
    let available: Vec<AvailableGame> = games
        .iter()
        .filter(|(_, g)| g.status == GameStatus::WaitingForOpponent)
        .map(|(id, g)| AvailableGame {
            game_id: *id,
            game_type: g.game_type,
            amount_sat: g.amount_sat,
            created_at_secs: g.created_at.elapsed().as_secs(),
        })
        .collect();

    Json(AvailableGamesResponse { games: available })
}

async fn create_game(
    State(state): State<Arc<OracleState>>,
    Json(req): Json<CreateGameRequest>,
) -> Json<CreateGameResponse> {
    let game_id = GameId::new();
    let commitment_point = state.generate_commitment_point(&game_id);

    // Generate Oracle secret if needed
    let (oracle_secret, oracle_commitment) = if req.game_type.requires_oracle_secret() {
        let secret = OracleSecret::random();
        let commitment = secret.commitment();
        (Some(secret), Some(commitment))
    } else {
        (None, None)
    };

    let game_state = GameState {
        game_type: req.game_type,
        amount_sat: req.amount_sat,
        status: GameStatus::WaitingForOpponent,
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

    state.games.write().unwrap().insert(game_id, game_state);

    info!("Created game {:?} of type {:?}", game_id, req.game_type);

    Json(CreateGameResponse {
        game_id,
        oracle_pubkey: hex::encode(state.public_key.serialize()),
        commitment_point: hex::encode(commitment_point.serialize()),
        oracle_commitment: oracle_commitment.map(hex::encode),
    })
}

async fn join_game(
    State(state): State<Arc<OracleState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<JoinGameRequest>,
) -> Result<Json<JoinGameResponse>, AppError> {
    let mut games = state.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    if game.status != GameStatus::WaitingForOpponent {
        return Err(AppError::from("Game is not available to join"));
    }

    game.player_b_id = Some(req.player_b_id);
    game.status = GameStatus::InProgress;

    info!("Player {:?} joined game {:?}", req.player_b_id, game_id);

    Ok(Json(JoinGameResponse {
        status: "joined".to_string(),
        oracle_pubkey: hex::encode(state.public_key.serialize()),
        commitment_point: hex::encode(game.commitment_point.serialize()),
        oracle_commitment: game.oracle_commitment.map(hex::encode),
    }))
}

async fn submit_invoice(
    State(state): State<Arc<OracleState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitInvoiceRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    match req.player {
        Player::A => game.invoice_a = Some(req.payment_hash),
        Player::B => game.invoice_b = Some(req.payment_hash),
    }

    Ok(Json(StatusResponse {
        status: "invoice_received".to_string(),
    }))
}

async fn get_invoice(
    State(state): State<Arc<OracleState>>,
    Path((game_id, player)): Path<(GameId, String)>,
) -> Result<Json<InvoiceResponse>, AppError> {
    let games = state.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    let payment_hash = match player.as_str() {
        "A" | "a" => game.invoice_a.ok_or(AppError::from("Invoice A not submitted"))?,
        "B" | "b" => game.invoice_b.ok_or(AppError::from("Invoice B not submitted"))?,
        _ => return Err(AppError::from("Invalid player")),
    };

    Ok(Json(InvoiceResponse { payment_hash }))
}

async fn submit_encrypted_preimage(
    State(state): State<Arc<OracleState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitEncryptedPreimageRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    match req.player {
        Player::A => game.encrypted_preimage_a = Some(req.encrypted_preimage),
        Player::B => game.encrypted_preimage_b = Some(req.encrypted_preimage),
    }

    Ok(Json(StatusResponse {
        status: "encrypted_preimage_received".to_string(),
    }))
}

async fn get_encrypted_preimage(
    State(state): State<Arc<OracleState>>,
    Path((game_id, player)): Path<(GameId, String)>,
) -> Result<Json<EncryptedPreimageResponse>, AppError> {
    let games = state.games.read().unwrap();
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

async fn submit_commit(
    State(state): State<Arc<OracleState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitCommitRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.games.write().unwrap();
    let game = games.get_mut(&game_id).ok_or(AppError::from("Game not found"))?;

    match req.player {
        Player::A => game.commit_a = Some(req.commitment),
        Player::B => game.commit_b = Some(req.commitment),
    }

    Ok(Json(StatusResponse {
        status: "commitment_received".to_string(),
    }))
}

async fn submit_reveal(
    State(state): State<Arc<OracleState>>,
    Path(game_id): Path<GameId>,
    Json(req): Json<SubmitRevealRequest>,
) -> Result<Json<StatusResponse>, AppError> {
    let mut games = state.games.write().unwrap();
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

        // Judge the game
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
        game.status = GameStatus::Completed;

        // Sign the result (simplified - in real implementation would use proper Schnorr)
        let mut sig = [0u8; 64];
        let msg = format!("{}:{}", game_id, result.as_str());
        let hash = sha2::Sha256::digest(msg.as_bytes());
        sig[..32].copy_from_slice(&hash);

        game.signature = Some(sig);

        info!("Game {:?} completed with result: {:?}", game_id, result);

        Ok(Json(StatusResponse {
            status: "game_complete".to_string(),
        }))
    } else {
        Ok(Json(StatusResponse {
            status: "waiting_for_opponent".to_string(),
        }))
    }
}

async fn get_result(
    State(state): State<Arc<OracleState>>,
    Path(game_id): Path<GameId>,
) -> Result<Json<GameResultResponse>, AppError> {
    let games = state.games.read().unwrap();
    let game = games.get(&game_id).ok_or(AppError::from("Game not found"))?;

    if game.status != GameStatus::Completed {
        return Ok(Json(GameResultResponse {
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

    Ok(Json(GameResultResponse {
        status: "completed".to_string(),
        result: game.result,
        signature: game.signature.map(hex::encode),
        game_data,
    }))
}

fn create_router(state: Arc<OracleState>) -> Router {
    Router::new()
        .route("/oracle/pubkey", get(get_pubkey))
        .route("/games/available", get(get_available_games))
        .route("/game/create", post(create_game))
        .route("/game/:game_id/join", post(join_game))
        .route("/game/:game_id/invoice", post(submit_invoice))
        .route("/game/:game_id/invoice/:player", get(get_invoice))
        .route(
            "/game/:game_id/encrypted-preimage",
            post(submit_encrypted_preimage),
        )
        .route(
            "/game/:game_id/encrypted-preimage/:player",
            get(get_encrypted_preimage),
        )
        .route("/game/:game_id/commit", post(submit_commit))
        .route("/game/:game_id/reveal", post(submit_reveal))
        .route("/game/:game_id/result", get(get_result))
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

    let state = Arc::new(OracleState::new());

    info!(
        "Oracle public key: {}",
        hex::encode(state.public_key.serialize())
    );

    let app = create_router(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Oracle service listening on http://0.0.0.0:3000");

    axum::serve(listener, app).await.unwrap();
}
