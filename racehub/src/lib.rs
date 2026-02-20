use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine;
use chrono::Utc;
use race_protocol::{
    ArtifactSummary, ErrorResponse, LoginRequest, LoginResponse, RegisterRequest,
    ServerCapabilities, UploadArtifactRequest, UploadArtifactResponse, UserInfo,
};
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

const LOCAL_USER_ID: i64 = 1;
const LOCAL_USERNAME: &str = "local";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Required,
    Disabled,
}

impl AuthMode {
    pub fn from_env(value: &str) -> Self {
        match value {
            "disabled" => Self::Disabled,
            _ => Self::Required,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Required => "server",
            Self::Disabled => "standalone",
        }
    }

    pub fn auth_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind: String,
    pub db_path: PathBuf,
    pub artifacts_dir: PathBuf,
    pub static_dir: Option<PathBuf>,
    pub auth_mode: AuthMode,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8787".to_string(),
            db_path: PathBuf::from("racehub.db"),
            artifacts_dir: PathBuf::from("racehub_artifacts"),
            static_dir: Some(PathBuf::from("web-dist")),
            auth_mode: AuthMode::Required,
        }
    }
}

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
    artifacts_dir: PathBuf,
    auth_mode: AuthMode,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

pub async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&config.artifacts_dir)?;
    let conn = Connection::open(&config.db_path)?;
    run_migrations(&conn)?;
    ensure_local_user(&conn)?;

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        artifacts_dir: config.artifacts_dir,
        auth_mode: config.auth_mode,
    };

    let mut app = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/me", get(me))
        .route(
            "/api/v1/artifacts",
            get(list_artifacts).post(upload_artifact),
        )
        .route("/api/v1/artifacts/{id}", get(download_artifact))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    if let Some(dir) = &config.static_dir {
        app = app.nest_service("/", ServeDir::new(dir));
    }

    let addr: SocketAddr = config.bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn capabilities(State(state): State<AppState>) -> Json<ServerCapabilities> {
    Json(ServerCapabilities {
        auth_required: state.auth_mode.auth_required(),
        mode: state.auth_mode.as_str().to_string(),
    })
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<UserInfo>, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Err(ApiError::bad_request("auth is disabled in standalone mode"));
    }

    if payload.username.trim().is_empty() {
        return Err(ApiError::bad_request("username must not be empty"));
    }
    if payload.password.len() < 8 {
        return Err(ApiError::bad_request("password must be at least 8 chars"));
    }

    let hash = hash_password(&payload.password)?;
    let now = now_utc();

    let db = state.db.lock().await;
    let inserted = db.execute(
        "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
        params![payload.username.trim(), hash, now],
    );

    if let Err(err) = inserted {
        if err.to_string().contains("UNIQUE") {
            return Err(ApiError::bad_request("username already exists"));
        }
        return Err(ApiError::internal(format!("failed to create user: {err}")));
    }

    Ok(Json(UserInfo {
        id: db.last_insert_rowid(),
        username: payload.username.trim().to_string(),
    }))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Err(ApiError::bad_request("auth is disabled in standalone mode"));
    }

    let username = payload.username.trim();
    if username.is_empty() {
        return Err(ApiError::bad_request("username must not be empty"));
    }

    let db = state.db.lock().await;
    let user_row: Option<(i64, String)> = db
        .query_row(
            "SELECT id, password_hash FROM users WHERE username = ?1",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query user: {e}")))?;

    let Some((user_id, password_hash)) = user_row else {
        return Err(ApiError::unauthorized("invalid credentials"));
    };

    verify_password(&payload.password, &password_hash)?;

    let token = generate_token();
    db.execute(
        "INSERT INTO sessions (token, user_id, created_at) VALUES (?1, ?2, ?3)",
        params![token, user_id, now_utc()],
    )
    .map_err(|e| ApiError::internal(format!("failed to create session: {e}")))?;

    Ok(Json(LoginResponse {
        token,
        user: UserInfo {
            id: user_id,
            username: username.to_string(),
        },
    }))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Ok(StatusCode::NO_CONTENT);
    }
    let token = bearer_token(&headers)?;
    let db = state.db.lock().await;
    db.execute("DELETE FROM sessions WHERE token = ?1", params![token])
        .map_err(|e| ApiError::internal(format!("failed to logout: {e}")))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<UserInfo>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    Ok(Json(user))
}

async fn list_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ArtifactSummary>>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;

    let mut sql =
        "SELECT id, owner_user_id, name, note, target, created_at FROM artifacts".to_string();
    if state.auth_mode == AuthMode::Required {
        sql.push_str(" WHERE owner_user_id = ?1");
    }
    sql.push_str(" ORDER BY created_at DESC");

    let mut stmt = db
        .prepare(&sql)
        .map_err(|e| ApiError::internal(format!("failed to prepare artifact query: {e}")))?;

    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(ArtifactSummary {
            id: row.get(0)?,
            owner_user_id: row.get(1)?,
            name: row.get(2)?,
            note: row.get(3)?,
            target: row.get(4)?,
            created_at: row.get(5)?,
        })
    };

    let rows = if state.auth_mode == AuthMode::Required {
        stmt.query_map(params![user.id], mapper)
    } else {
        stmt.query_map([], mapper)
    }
    .map_err(|e| ApiError::internal(format!("failed to query artifacts: {e}")))?;

    let mut out = Vec::new();
    for item in rows {
        out.push(
            item.map_err(|e| ApiError::internal(format!("failed to read artifact row: {e}")))?,
        );
    }

    Ok(Json(out))
}

async fn upload_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadArtifactRequest>,
) -> Result<Json<UploadArtifactResponse>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request("artifact name must not be empty"));
    }
    if payload.target.trim().is_empty() {
        return Err(ApiError::bad_request("artifact target must not be empty"));
    }

    let elf_bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.elf_base64.as_bytes())
        .map_err(|e| ApiError::bad_request(format!("invalid elf base64: {e}")))?;
    if elf_bytes.is_empty() {
        return Err(ApiError::bad_request("elf payload must not be empty"));
    }

    let db = state.db.lock().await;
    let now = now_utc();
    db.execute(
        "INSERT INTO artifacts (owner_user_id, name, note, target, elf_path, created_at) VALUES (?1, ?2, ?3, ?4, '', ?5)",
        params![user.id, payload.name.trim(), payload.note, payload.target.trim(), now],
    )
    .map_err(|e| ApiError::internal(format!("failed to create artifact row: {e}")))?;

    let artifact_id = db.last_insert_rowid();
    let artifact_name = format!("artifact_{artifact_id}.elf");
    let artifact_path = state.artifacts_dir.join(&artifact_name);

    if let Err(error) = std::fs::write(&artifact_path, elf_bytes) {
        let _ = db.execute("DELETE FROM artifacts WHERE id = ?1", params![artifact_id]);
        return Err(ApiError::internal(format!(
            "failed to write artifact file: {error}"
        )));
    }

    db.execute(
        "UPDATE artifacts SET elf_path = ?1 WHERE id = ?2",
        params![artifact_name, artifact_id],
    )
    .map_err(|e| ApiError::internal(format!("failed to update artifact path: {e}")))?;

    Ok(Json(UploadArtifactResponse { artifact_id }))
}

async fn download_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(artifact_id): AxumPath<i64>,
) -> Result<Response, ApiError> {
    let user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;

    let row: Option<(i64, String)> = db
        .query_row(
            "SELECT owner_user_id, elf_path FROM artifacts WHERE id = ?1",
            params![artifact_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query artifact: {e}")))?;

    let Some((owner_user_id, rel_path)) = row else {
        return Err(ApiError::not_found("artifact not found"));
    };

    if state.auth_mode == AuthMode::Required && owner_user_id != user.id {
        return Err(ApiError::unauthorized(
            "artifact is not owned by current user",
        ));
    }

    let full_path = state.artifacts_dir.join(rel_path);
    let bytes = std::fs::read(&full_path)
        .map_err(|e| ApiError::internal(format!("failed to read artifact file: {e}")))?;

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/octet-stream")],
        bytes,
    )
        .into_response())
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<UserInfo, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Ok(UserInfo {
            id: LOCAL_USER_ID,
            username: LOCAL_USERNAME.to_string(),
        });
    }

    let token = bearer_token(headers)?;
    let db = state.db.lock().await;

    let user: Option<UserInfo> = db
        .query_row(
            "SELECT u.id, u.username FROM sessions s JOIN users u ON s.user_id = u.id WHERE s.token = ?1",
            params![token],
            |row| {
                Ok(UserInfo {
                    id: row.get(0)?,
                    username: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to lookup session: {e}")))?;

    user.ok_or_else(|| ApiError::unauthorized("invalid or expired session token"))
}

fn bearer_token(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = headers
        .get("Authorization")
        .ok_or_else(|| ApiError::unauthorized("missing Authorization header"))?
        .to_str()
        .map_err(|_| ApiError::unauthorized("invalid Authorization header"))?;

    let token = value
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("Authorization must use Bearer token"))?
        .trim();

    if token.is_empty() {
        return Err(ApiError::unauthorized("missing bearer token"));
    }

    Ok(token.to_string())
}

fn generate_token() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    hex::encode(bytes)
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let mut rng = rand::rng();
    let salt_bytes: [u8; 16] = rng.random();
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|e| ApiError::internal(format!("failed to encode password salt: {e}")))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::internal(format!("password hash failed: {e}")))
}

fn verify_password(password: &str, hash: &str) -> Result<(), ApiError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| ApiError::internal(format!("invalid password hash in database: {e}")))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| ApiError::unauthorized("invalid credentials"))
}

fn now_utc() -> String {
    Utc::now().to_rfc3339()
}

fn ensure_local_user(conn: &Connection) -> Result<(), rusqlite::Error> {
    let now = now_utc();
    conn.execute(
        "INSERT OR IGNORE INTO users (id, username, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![LOCAL_USER_ID, LOCAL_USERNAME, "", now],
    )?;
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            token TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            note TEXT,
            target TEXT NOT NULL,
            elf_path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(owner_user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        ",
    )
}

#[allow(dead_code)]
fn _ensure_under(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}
