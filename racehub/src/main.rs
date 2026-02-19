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
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine;
use chrono::Utc;
use race_protocol::{
    ArtifactSummary, CreateScriptRequest, CreateScriptVersionRequest, ErrorResponse, LoginRequest,
    LoginResponse, PublishRaceRecordRequest, RaceRecordSummary, RegisterRequest, ScriptSummary,
    ScriptVersionSummary, UploadArtifactRequest, UploadArtifactResponse, UserInfo,
};
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::info;

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
    artifacts_dir: PathBuf,
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

#[derive(Deserialize)]
struct RaceRecordQuery {
    user_id: Option<i64>,
    track_id: Option<String>,
}

#[derive(Deserialize)]
struct ArtifactListQuery {
    script_id: Option<i64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "racehub=info,tower_http=info".into()),
        )
        .init();

    let db_path = std::env::var("RACEHUB_DB_PATH").unwrap_or_else(|_| "racehub.db".to_string());
    let artifacts_dir = std::env::var("RACEHUB_ARTIFACTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("racehub_artifacts"));
    std::fs::create_dir_all(&artifacts_dir)?;

    let conn = Connection::open(&db_path)?;
    run_migrations(&conn)?;

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        artifacts_dir,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/me", get(me))
        .route("/api/v1/scripts", get(list_scripts).post(create_script))
        .route("/api/v1/scripts/{id}/versions", post(create_script_version))
        .route("/api/v1/artifacts", get(list_artifacts))
        .route("/api/v1/artifacts/upload", post(upload_artifact))
        .route("/api/v1/artifacts/{id}", get(download_artifact))
        .route(
            "/api/v1/race-records",
            post(create_race_record).get(list_race_records),
        )
        .route("/api/v1/race-records/{id}", get(get_race_record))
        .nest_service("/", ServeDir::new("web-dist"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let bind = std::env::var("RACEHUB_BIND").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let addr: SocketAddr = bind.parse()?;
    info!(%addr, "racehub listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<UserInfo>, ApiError> {
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

    let id = db.last_insert_rowid();
    Ok(Json(UserInfo {
        id,
        username: payload.username.trim().to_string(),
    }))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let username = payload.username.trim();
    if username.is_empty() {
        return Err(ApiError::bad_request("username must not be empty"));
    }

    let now = now_utc();
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
        params![token, user_id, now],
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

async fn list_scripts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ScriptSummary>>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;
    let mut stmt = db
        .prepare(
            "SELECT id, name, language, created_at, updated_at FROM scripts WHERE user_id = ?1 ORDER BY updated_at DESC",
        )
        .map_err(|e| ApiError::internal(format!("failed to prepare script query: {e}")))?;

    let rows = stmt
        .query_map(params![user.id], |row| {
            Ok(ScriptSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                language: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| ApiError::internal(format!("failed to query scripts: {e}")))?;

    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| ApiError::internal(format!("failed to read script row: {e}")))?);
    }

    Ok(Json(out))
}

async fn create_script(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateScriptRequest>,
) -> Result<Json<ScriptSummary>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request("script name must not be empty"));
    }

    let now = now_utc();
    let db = state.db.lock().await;
    db.execute(
        "INSERT INTO scripts (user_id, name, language, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user.id, payload.name.trim(), payload.language.trim(), now, now],
    )
    .map_err(|e| ApiError::internal(format!("failed to create script: {e}")))?;

    Ok(Json(ScriptSummary {
        id: db.last_insert_rowid(),
        name: payload.name.trim().to_string(),
        language: payload.language.trim().to_string(),
        created_at: now.clone(),
        updated_at: now,
    }))
}

async fn create_script_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(script_id): AxumPath<i64>,
    Json(payload): Json<CreateScriptVersionRequest>,
) -> Result<Json<ScriptVersionSummary>, ApiError> {
    let user = authenticate(&state, &headers).await?;

    let db = state.db.lock().await;
    let owner: Option<i64> = db
        .query_row(
            "SELECT user_id FROM scripts WHERE id = ?1",
            params![script_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query script owner: {e}")))?;

    let Some(owner_id) = owner else {
        return Err(ApiError::not_found("script not found"));
    };
    if owner_id != user.id {
        return Err(ApiError::unauthorized(
            "script does not belong to current user",
        ));
    }

    let current_max_version: Option<i64> = db
        .query_row(
            "SELECT MAX(version) FROM script_versions WHERE script_id = ?1",
            params![script_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query script version: {e}")))?;

    let version = current_max_version.unwrap_or(0) + 1;
    let now = now_utc();

    db.execute(
        "INSERT INTO script_versions (script_id, version, commit_hash, source_bundle_path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            script_id,
            version,
            payload.commit_hash,
            payload.source_bundle_path,
            now
        ],
    )
    .map_err(|e| ApiError::internal(format!("failed to create script version: {e}")))?;

    db.execute(
        "UPDATE scripts SET updated_at = ?1 WHERE id = ?2",
        params![now, script_id],
    )
    .map_err(|e| ApiError::internal(format!("failed to update script timestamp: {e}")))?;

    Ok(Json(ScriptVersionSummary {
        id: db.last_insert_rowid(),
        script_id,
        version,
        commit_hash: payload.commit_hash,
        source_bundle_path: payload.source_bundle_path,
        created_at: now,
    }))
}

async fn upload_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadArtifactRequest>,
) -> Result<Json<UploadArtifactResponse>, ApiError> {
    let user = authenticate(&state, &headers).await?;

    let elf_bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.elf_base64.as_bytes())
        .map_err(|e| ApiError::bad_request(format!("invalid elf base64: {e}")))?;

    if elf_bytes.is_empty() {
        return Err(ApiError::bad_request("elf payload must not be empty"));
    }

    let db = state.db.lock().await;
    let script_owner: Option<i64> = db
        .query_row(
            "SELECT s.user_id FROM script_versions sv JOIN scripts s ON sv.script_id = s.id WHERE sv.id = ?1",
            params![payload.script_version_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query script version ownership: {e}")))?;

    let Some(script_owner_id) = script_owner else {
        return Err(ApiError::not_found("script version not found"));
    };

    if script_owner_id != user.id {
        return Err(ApiError::unauthorized(
            "script version does not belong to current user",
        ));
    }

    let now = now_utc();
    db.execute(
        "INSERT INTO artifacts (script_version_id, target, elf_path, build_meta_json, created_at) VALUES (?1, ?2, '', ?3, ?4)",
        params![
            payload.script_version_id,
            payload.target.trim(),
            payload.build_meta_json,
            now
        ],
    )
    .map_err(|e| ApiError::internal(format!("failed to create artifact row: {e}")))?;

    let artifact_id = db.last_insert_rowid();
    let artifact_name = format!("artifact_{artifact_id}.elf");
    let artifact_path = state.artifacts_dir.join(&artifact_name);

    std::fs::write(&artifact_path, elf_bytes)
        .map_err(|e| ApiError::internal(format!("failed to write artifact file: {e}")))?;

    db.execute(
        "UPDATE artifacts SET elf_path = ?1 WHERE id = ?2",
        params![artifact_name, artifact_id],
    )
    .map_err(|e| ApiError::internal(format!("failed to update artifact path: {e}")))?;

    Ok(Json(UploadArtifactResponse { artifact_id }))
}

async fn list_artifacts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ArtifactListQuery>,
) -> Result<Json<Vec<ArtifactSummary>>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;
    let mut stmt = db
        .prepare(
            "
            SELECT a.id, a.script_version_id, a.target, a.created_at
            FROM artifacts a
            JOIN script_versions sv ON a.script_version_id = sv.id
            JOIN scripts s ON sv.script_id = s.id
            WHERE s.user_id = ?1
            ORDER BY a.created_at DESC
            ",
        )
        .map_err(|e| ApiError::internal(format!("failed to prepare artifact query: {e}")))?;

    let rows = stmt
        .query_map(params![user.id], |row| {
            Ok(ArtifactSummary {
                id: row.get(0)?,
                script_version_id: row.get(1)?,
                target: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| ApiError::internal(format!("failed to query artifacts: {e}")))?;

    let mut out = Vec::new();
    for item in rows {
        out.push(
            item.map_err(|e| ApiError::internal(format!("failed to read artifact row: {e}")))?,
        );
    }

    if let Some(script_id) = query.script_id {
        let version_ids: Vec<i64> = db
            .prepare("SELECT id FROM script_versions WHERE script_id = ?1")
            .and_then(|mut stmt| {
                stmt.query_map(params![script_id], |row| row.get(0))
                    .map(|iter| iter.collect::<Result<Vec<i64>, _>>())
            })
            .map_err(|e| ApiError::internal(format!("failed to query script versions: {e}")))?
            .map_err(|e| ApiError::internal(format!("failed to read script versions: {e}")))?;
        out.retain(|artifact| version_ids.contains(&artifact.script_version_id));
    }

    Ok(Json(out))
}

async fn download_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(artifact_id): AxumPath<i64>,
) -> Result<Response, ApiError> {
    let _user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;

    let rel_path: Option<String> = db
        .query_row(
            "SELECT elf_path FROM artifacts WHERE id = ?1",
            params![artifact_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query artifact path: {e}")))?;

    let Some(rel_path) = rel_path else {
        return Err(ApiError::not_found("artifact not found"));
    };

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

async fn create_race_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PublishRaceRecordRequest>,
) -> Result<Json<RaceRecordSummary>, ApiError> {
    let user = authenticate(&state, &headers).await?;
    if payload.track_id.trim().is_empty() {
        return Err(ApiError::bad_request("track_id must not be empty"));
    }

    let now = now_utc();
    let db = state.db.lock().await;

    db.execute(
        "INSERT INTO race_records (user_id, track_id, result_json, replay_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            user.id,
            payload.track_id.trim(),
            payload.result_json,
            payload.replay_json,
            now
        ],
    )
    .map_err(|e| ApiError::internal(format!("failed to create race record: {e}")))?;

    Ok(Json(RaceRecordSummary {
        id: db.last_insert_rowid(),
        user_id: user.id,
        track_id: payload.track_id.trim().to_string(),
        result_json: payload.result_json,
        replay_json: payload.replay_json,
        created_at: now,
    }))
}

async fn get_race_record(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(record_id): AxumPath<i64>,
) -> Result<Json<RaceRecordSummary>, ApiError> {
    let _user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;

    let row: Option<RaceRecordSummary> = db
        .query_row(
            "SELECT id, user_id, track_id, result_json, replay_json, created_at FROM race_records WHERE id = ?1",
            params![record_id],
            |row| {
                Ok(RaceRecordSummary {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    track_id: row.get(2)?,
                    result_json: row.get(3)?,
                    replay_json: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query race record: {e}")))?;

    let Some(record) = row else {
        return Err(ApiError::not_found("race record not found"));
    };

    Ok(Json(record))
}

async fn list_race_records(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RaceRecordQuery>,
) -> Result<Json<Vec<RaceRecordSummary>>, ApiError> {
    let _user = authenticate(&state, &headers).await?;

    let db = state.db.lock().await;
    let mut stmt = db
        .prepare(
            "SELECT id, user_id, track_id, result_json, replay_json, created_at FROM race_records ORDER BY created_at DESC",
        )
        .map_err(|e| ApiError::internal(format!("failed to prepare race record list query: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(RaceRecordSummary {
                id: row.get(0)?,
                user_id: row.get(1)?,
                track_id: row.get(2)?,
                result_json: row.get(3)?,
                replay_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| ApiError::internal(format!("failed to query race records: {e}")))?;

    let mut out = Vec::new();
    for item in rows {
        out.push(item.map_err(|e| ApiError::internal(format!("failed to read race row: {e}")))?);
    }

    if let Some(user_id) = query.user_id {
        out.retain(|r| r.user_id == user_id);
    }
    if let Some(track_id) = query.track_id {
        out.retain(|r| r.track_id == track_id);
    }

    Ok(Json(out))
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<UserInfo, ApiError> {
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

        CREATE TABLE IF NOT EXISTS scripts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            language TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS script_versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            script_id INTEGER NOT NULL,
            version INTEGER NOT NULL,
            commit_hash TEXT,
            source_bundle_path TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(script_id, version),
            FOREIGN KEY(script_id) REFERENCES scripts(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            script_version_id INTEGER NOT NULL,
            target TEXT NOT NULL,
            elf_path TEXT NOT NULL,
            build_meta_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY(script_version_id) REFERENCES script_versions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS race_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            track_id TEXT NOT NULL,
            result_json TEXT NOT NULL,
            replay_json TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        ",
    )
}

#[allow(dead_code)]
fn _ensure_under(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}
