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
    Form, Json, Router,
    extract::{OriginalUri, Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, patch, post},
};
use base64::Engine;
use chrono::Utc;
use race_protocol::{
    ArtifactSummary, ErrorResponse, LoginRequest, LoginResponse, RegisterRequest,
    ServerCapabilities, UpdateArtifactVisibilityRequest, UploadArtifactRequest,
    UploadArtifactResponse, UserInfo,
};
use rand::Rng;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::{debug, info, warn};

const LOCAL_USER_ID: i64 = 1;
const LOCAL_USERNAME: &str = "local";
const COOKIE_NAME: &str = "racehub_session";

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
    pub cookie_secure: bool,
    pub registration_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8787".to_string(),
            db_path: PathBuf::from("racehub.db"),
            artifacts_dir: PathBuf::from("racehub_artifacts"),
            static_dir: Some(PathBuf::from("web-dist")),
            auth_mode: AuthMode::Required,
            cookie_secure: false,
            registration_enabled: true,
        }
    }
}

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Connection>>,
    artifacts_dir: PathBuf,
    static_dir: Option<PathBuf>,
    auth_mode: AuthMode,
    cookie_secure: bool,
    registration_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct WebLoginQuery {
    next: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebLoginForm {
    username: String,
    password: String,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebRegisterQuery {
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebRegisterForm {
    username: String,
    password: String,
    next: Option<String>,
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

    fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
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
    info!(
        bind = %config.bind,
        auth_mode = %config.auth_mode.as_str(),
        db_path = %config.db_path.display(),
        artifacts_dir = %config.artifacts_dir.display(),
        static_dir = ?config.static_dir.as_ref().map(|p| p.display().to_string()),
        registration_enabled = config.registration_enabled,
        "starting racehub server"
    );

    std::fs::create_dir_all(&config.artifacts_dir)?;
    let conn = Connection::open(&config.db_path)?;
    run_migrations(&conn)?;
    ensure_local_user(&conn)?;

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        artifacts_dir: config.artifacts_dir,
        static_dir: config.static_dir.clone(),
        auth_mode: config.auth_mode,
        cookie_secure: config.cookie_secure,
        registration_enabled: config.registration_enabled,
    };

    let app = build_app(state, config.static_dir);

    let addr: SocketAddr = config.bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "racehub listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("racehub server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigint = signal(SignalKind::interrupt()).expect("register SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate()).expect("register SIGTERM handler");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => info!("shutdown signal received (ctrl_c)"),
            _ = sigint.recv() => info!("shutdown signal received (SIGINT)"),
            _ = sigterm.recv() => info!("shutdown signal received (SIGTERM)"),
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        info!("shutdown signal received (ctrl_c)");
    }
}

fn build_app(state: AppState, static_dir: Option<PathBuf>) -> Router {
    let mut app = Router::new()
        .route("/", get(web_game_entry))
        .route("/index.html", get(web_game_entry))
        .route("/login", get(web_login_get).post(web_login_post))
        .route("/register", get(web_register_get).post(web_register_post))
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
        .route(
            "/api/v1/artifacts/{id}",
            get(download_artifact).delete(delete_artifact),
        )
        .route(
            "/api/v1/artifacts/{id}/visibility",
            patch(update_artifact_visibility),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    if let Some(dir) = static_dir {
        info!(static_dir = %dir.display(), "serving static files");
        app = app.fallback_service(ServeDir::new(dir));
    } else {
        warn!("static file serving disabled (RACEHUB_STATIC_DIR empty)");
    }

    app
}

async fn healthz() -> &'static str {
    "ok"
}

async fn capabilities(State(state): State<AppState>) -> Json<ServerCapabilities> {
    Json(ServerCapabilities {
        auth_required: state.auth_mode.auth_required(),
        mode: state.auth_mode.as_str().to_string(),
        registration_enabled: state.registration_enabled,
    })
}

async fn web_game_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    if state.auth_mode == AuthMode::Required && authenticate(&state, &headers).await.is_err() {
        let next = sanitize_next(
            uri.path_and_query()
                .map(|v| v.as_str())
                .unwrap_or("/index.html"),
        );
        return render_login_page(next, None, None, state.registration_enabled).into_response();
    }

    match load_index_html(&state) {
        Ok(html) => Html(html).into_response(),
        Err(err) => err.into_response(),
    }
}

async fn web_login_get(
    State(state): State<AppState>,
    Query(query): Query<WebLoginQuery>,
) -> impl IntoResponse {
    let next = sanitize_next(query.next.as_deref().unwrap_or("/"));
    let error = match query.error.as_deref() {
        Some("registration_disabled") => Some("Registration is disabled"),
        _ => None,
    };
    render_login_page(next, None, error, state.registration_enabled)
}

async fn web_login_post(
    State(state): State<AppState>,
    Form(payload): Form<WebLoginForm>,
) -> Response {
    if state.auth_mode == AuthMode::Disabled {
        let target = sanitize_next(payload.next.as_deref().unwrap_or("/"));
        return Redirect::to(target).into_response();
    }

    let username = payload.username.trim();
    let next = sanitize_next(payload.next.as_deref().unwrap_or("/"));

    match create_session_for_credentials(&state, username, &payload.password).await {
        Ok((_user, token)) => {
            let cookie = session_cookie(&token, state.cookie_secure);
            (
                StatusCode::SEE_OTHER,
                [
                    (header::SET_COOKIE, cookie),
                    (
                        header::LOCATION,
                        HeaderValue::from_str(next).expect("sanitized redirect target"),
                    ),
                ],
            )
                .into_response()
        }
        Err(_) => {
            warn!(username, "web login failed");
            (
                StatusCode::UNAUTHORIZED,
                render_login_page(
                    next,
                    Some(username),
                    Some("Invalid username or password"),
                    state.registration_enabled,
                ),
            )
                .into_response()
        }
    }
}

async fn web_register_get(
    State(state): State<AppState>,
    Query(query): Query<WebRegisterQuery>,
) -> Response {
    let next = sanitize_next(query.next.as_deref().unwrap_or("/"));
    if state.auth_mode == AuthMode::Disabled || !state.registration_enabled {
        return Redirect::to(next_login_with_error(next, Some("registration_disabled")).as_str())
            .into_response();
    }
    render_register_page(next, None, None).into_response()
}

async fn web_register_post(
    State(state): State<AppState>,
    Form(payload): Form<WebRegisterForm>,
) -> Response {
    let next = sanitize_next(payload.next.as_deref().unwrap_or("/"));
    if state.auth_mode == AuthMode::Disabled || !state.registration_enabled {
        return Redirect::to(next_login_with_error(next, Some("registration_disabled")).as_str())
            .into_response();
    }

    let username = payload.username.trim();
    match create_user_with_password(&state, username, &payload.password).await {
        Ok(_user) => {
            match create_session_for_credentials(&state, username, &payload.password).await {
                Ok((_user, token)) => {
                    let cookie = session_cookie(&token, state.cookie_secure);
                    (
                        StatusCode::SEE_OTHER,
                        [
                            (header::SET_COOKIE, cookie),
                            (
                                header::LOCATION,
                                HeaderValue::from_str(next).expect("sanitized redirect target"),
                            ),
                        ],
                    )
                        .into_response()
                }
                Err(err) => err.into_response(),
            }
        }
        Err(err) => {
            let status = if err.status == StatusCode::INTERNAL_SERVER_ERROR {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::BAD_REQUEST
            };
            (
                status,
                render_register_page(next, Some(username), Some(&err.message)),
            )
                .into_response()
        }
    }
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<UserInfo>, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Err(ApiError::bad_request("auth is disabled in standalone mode"));
    }
    if !state.registration_enabled {
        return Err(ApiError::forbidden("registration is disabled"));
    }
    let user =
        create_user_with_password(&state, payload.username.trim(), &payload.password).await?;
    Ok(Json(user))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Err(ApiError::bad_request("auth is disabled in standalone mode"));
    }

    let username = payload.username.trim();
    let (user, token) = create_session_for_credentials(&state, username, &payload.password).await?;
    let login = LoginResponse {
        token: token.clone(),
        user,
    };

    let cookie = session_cookie(&token, state.cookie_secure);
    Ok((StatusCode::OK, [(header::SET_COOKIE, cookie)], Json(login)).into_response())
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let mut removed = false;
    if let Some(token) = bearer_token_opt(&headers) {
        let db = state.db.lock().await;
        db.execute("DELETE FROM sessions WHERE token = ?1", params![token])
            .map_err(|e| ApiError::internal(format!("failed to logout bearer token: {e}")))?;
        removed = true;
    }

    if let Some(token) = session_cookie_token(&headers) {
        let db = state.db.lock().await;
        db.execute("DELETE FROM sessions WHERE token = ?1", params![token])
            .map_err(|e| ApiError::internal(format!("failed to logout cookie token: {e}")))?;
        removed = true;
    }

    if !removed {
        return Err(ApiError::unauthorized("missing auth token/session cookie"));
    }

    let clear_cookie = expired_session_cookie(state.cookie_secure);
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, clear_cookie)]).into_response())
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

    let mut sql = "SELECT a.id, a.owner_user_id, u.username, a.name, a.note, a.target, a.is_public, a.created_at FROM artifacts a JOIN users u ON u.id = a.owner_user_id".to_string();
    if state.auth_mode == AuthMode::Required {
        sql.push_str(" WHERE a.owner_user_id = ?1 OR a.is_public = 1");
    }
    sql.push_str(" ORDER BY a.created_at DESC");

    let mut stmt = db
        .prepare(&sql)
        .map_err(|e| ApiError::internal(format!("failed to prepare artifact query: {e}")))?;

    let mapper = |row: &rusqlite::Row<'_>| {
        let owner_user_id: i64 = row.get(1)?;
        Ok(ArtifactSummary {
            id: row.get(0)?,
            owner_user_id,
            owner_username: row.get(2)?,
            name: row.get(3)?,
            note: row.get(4)?,
            target: row.get(5)?,
            is_public: row.get::<_, i64>(6)? != 0,
            owned_by_me: owner_user_id == user.id,
            created_at: row.get(7)?,
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
        "INSERT INTO artifacts (owner_user_id, name, note, target, elf_path, is_public, created_at) VALUES (?1, ?2, ?3, ?4, '', 0, ?5)",
        params![
            user.id,
            payload.name.trim(),
            payload.note,
            payload.target.trim(),
            now
        ],
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

    info!(
        artifact_id,
        owner_user_id = user.id,
        artifact_name = payload.name.trim(),
        target = payload.target.trim(),
        is_public = false,
        "artifact uploaded"
    );
    Ok(Json(UploadArtifactResponse { artifact_id }))
}

async fn download_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(artifact_id): AxumPath<i64>,
) -> Result<Response, ApiError> {
    let user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;

    let row: Option<(i64, String, i64)> = db
        .query_row(
            "SELECT owner_user_id, elf_path, is_public FROM artifacts WHERE id = ?1",
            params![artifact_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query artifact: {e}")))?;

    let Some((owner_user_id, rel_path, is_public)) = row else {
        return Err(ApiError::not_found("artifact not found"));
    };

    if state.auth_mode == AuthMode::Required && owner_user_id != user.id && is_public == 0 {
        return Err(ApiError::unauthorized(
            "artifact is not owned by current user",
        ));
    }

    let full_path = state.artifacts_dir.join(rel_path);
    let bytes = std::fs::read(&full_path)
        .map_err(|e| ApiError::internal(format!("failed to read artifact file: {e}")))?;

    Ok((
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        )],
        bytes,
    )
        .into_response())
}

async fn delete_artifact(
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

    let relative = Path::new(&rel_path);
    if relative.is_absolute() || relative.components().count() != 1 {
        return Err(ApiError::internal("invalid artifact file path"));
    }

    let full_path = state.artifacts_dir.join(relative);
    if !full_path.starts_with(&state.artifacts_dir) {
        return Err(ApiError::internal("artifact path escaped storage root"));
    }

    match std::fs::remove_file(&full_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(ApiError::internal(format!(
                "failed to delete artifact file: {error}"
            )));
        }
    }

    db.execute("DELETE FROM artifacts WHERE id = ?1", params![artifact_id])
        .map_err(|e| ApiError::internal(format!("failed to delete artifact row: {e}")))?;

    info!(artifact_id, owner_user_id = user.id, "artifact deleted");
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn update_artifact_visibility(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(artifact_id): AxumPath<i64>,
    Json(payload): Json<UpdateArtifactVisibilityRequest>,
) -> Result<Response, ApiError> {
    let user = authenticate(&state, &headers).await?;
    let db = state.db.lock().await;

    let owner_user_id: Option<i64> = db
        .query_row(
            "SELECT owner_user_id FROM artifacts WHERE id = ?1",
            params![artifact_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| ApiError::internal(format!("failed to query artifact: {e}")))?;

    let Some(owner_user_id) = owner_user_id else {
        return Err(ApiError::not_found("artifact not found"));
    };

    if state.auth_mode == AuthMode::Required && owner_user_id != user.id {
        return Err(ApiError::unauthorized(
            "artifact is not owned by current user",
        ));
    }

    let is_public_i64 = if payload.is_public { 1 } else { 0 };
    db.execute(
        "UPDATE artifacts SET is_public = ?1 WHERE id = ?2",
        params![is_public_i64, artifact_id],
    )
    .map_err(|e| ApiError::internal(format!("failed to update artifact visibility: {e}")))?;

    info!(
        artifact_id,
        owner_user_id = user.id,
        is_public = payload.is_public,
        "artifact visibility updated"
    );

    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<UserInfo, ApiError> {
    if state.auth_mode == AuthMode::Disabled {
        return Ok(UserInfo {
            id: LOCAL_USER_ID,
            username: LOCAL_USERNAME.to_string(),
        });
    }

    let token = if let Some(token) = bearer_token_opt(headers) {
        token
    } else if let Some(token) = session_cookie_token(headers) {
        token
    } else {
        debug!("authentication failed: no bearer token or session cookie");
        return Err(ApiError::unauthorized(
            "missing Authorization bearer token or session cookie",
        ));
    };

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

    if user.is_none() {
        debug!("authentication failed: invalid or expired session");
    }
    user.ok_or_else(|| ApiError::unauthorized("invalid or expired session"))
}

async fn create_session_for_credentials(
    state: &AppState,
    username: &str,
    password: &str,
) -> Result<(UserInfo, String), ApiError> {
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

    verify_password(password, &password_hash)?;

    let token = generate_token();
    db.execute(
        "INSERT INTO sessions (token, user_id, created_at) VALUES (?1, ?2, ?3)",
        params![token, user_id, now_utc()],
    )
    .map_err(|e| ApiError::internal(format!("failed to create session: {e}")))?;

    Ok((
        UserInfo {
            id: user_id,
            username: username.to_string(),
        },
        token,
    ))
}

async fn create_user_with_password(
    state: &AppState,
    username: &str,
    password: &str,
) -> Result<UserInfo, ApiError> {
    if username.is_empty() {
        return Err(ApiError::bad_request("username must not be empty"));
    }
    if password.len() < 8 {
        return Err(ApiError::bad_request("password must be at least 8 chars"));
    }

    let hash = hash_password(password)?;
    let db = state.db.lock().await;
    let inserted = db.execute(
        "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
        params![username, hash, now_utc()],
    );

    if let Err(err) = inserted {
        if err.to_string().contains("UNIQUE") {
            return Err(ApiError::bad_request("username already exists"));
        }
        return Err(ApiError::internal(format!("failed to create user: {err}")));
    }

    Ok(UserInfo {
        id: db.last_insert_rowid(),
        username: username.to_string(),
    })
}

fn sanitize_next(next: &str) -> &str {
    if next.starts_with('/') && !next.starts_with("//") {
        next
    } else {
        "/"
    }
}

fn next_login_with_error(next: &str, error: Option<&str>) -> String {
    let mut url = format!("/login?next={}", urlencoding::encode(next));
    if let Some(error) = error {
        url.push_str("&error=");
        url.push_str(&urlencoding::encode(error));
    }
    url
}

fn render_login_page(
    next: &str,
    username: Option<&str>,
    error: Option<&str>,
    registration_enabled: bool,
) -> Html<String> {
    let escaped_next = escape_html(next);
    let escaped_username = escape_html(username.unwrap_or(""));
    let error_html = match error {
        Some(message) => format!("<p class=\"error\">{}</p>", escape_html(message)),
        None => String::new(),
    };
    let register_link = if registration_enabled {
        format!(
            "<p class=\"hint\"><a href=\"/register?next={}\">Create account</a></p>",
            urlencoding::encode(next)
        )
    } else {
        String::new()
    };
    Html(format!(
        "<!doctype html>\
         <html lang=\"en\">\
         <head>\
         <meta charset=\"utf-8\" />\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\
         <title>RaceHub Login</title>\
         <style>\
         body {{ margin:0; min-height:100vh; display:grid; place-items:center; font-family:system-ui,sans-serif; background:#0f1217; color:#f5f7fb; }}\
         .card {{ width:min(420px, calc(100vw - 2rem)); background:#171c24; border:1px solid #2a3240; border-radius:12px; padding:1.25rem; box-sizing:border-box; }}\
         h1 {{ margin:0 0 1rem 0; font-size:1.25rem; }}\
         label {{ display:block; margin:0 0 0.25rem 0; font-size:0.9rem; color:#c7d0dd; }}\
         input {{ width:100%; box-sizing:border-box; border:1px solid #3a4455; border-radius:8px; padding:0.6rem 0.75rem; margin:0 0 0.75rem 0; background:#0f141c; color:#f5f7fb; }}\
         button {{ width:100%; border:0; border-radius:8px; padding:0.65rem 0.75rem; font-weight:600; background:#6ad490; color:#131417; cursor:pointer; }}\
         .error {{ margin:0 0 0.75rem 0; color:#ff8d8d; font-size:0.9rem; }}\
         .hint {{ margin:0.75rem 0 0; color:#a9b5c5; font-size:0.8rem; }}\
         a {{ color:#9bd0ff; }}\
         </style>\
         </head>\
         <body>\
         <main class=\"card\">\
         <h1>Sign in to RaceHub</h1>\
         {error_html}\
         <form method=\"post\" action=\"/login\">\
         <input type=\"hidden\" name=\"next\" value=\"{escaped_next}\" />\
         <label for=\"username\">Username</label>\
         <input id=\"username\" name=\"username\" autocomplete=\"username\" required value=\"{escaped_username}\" />\
         <label for=\"password\">Password</label>\
         <input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"current-password\" required />\
         <button type=\"submit\">Log in</button>\
         </form>\
         {register_link}\
         <p class=\"hint\">You will be redirected back to the game after login.</p>\
         </main>\
         </body>\
         </html>"
    ))
}

fn render_register_page(next: &str, username: Option<&str>, error: Option<&str>) -> Html<String> {
    let escaped_next = escape_html(next);
    let escaped_username = escape_html(username.unwrap_or(""));
    let error_html = match error {
        Some(message) => format!("<p class=\"error\">{}</p>", escape_html(message)),
        None => String::new(),
    };
    Html(format!(
        "<!doctype html>\
         <html lang=\"en\">\
         <head>\
         <meta charset=\"utf-8\" />\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\
         <title>RaceHub Register</title>\
         <style>\
         body {{ margin:0; min-height:100vh; display:grid; place-items:center; font-family:system-ui,sans-serif; background:#0f1217; color:#f5f7fb; }}\
         .card {{ width:min(420px, calc(100vw - 2rem)); background:#171c24; border:1px solid #2a3240; border-radius:12px; padding:1.25rem; box-sizing:border-box; }}\
         h1 {{ margin:0 0 1rem 0; font-size:1.25rem; }}\
         label {{ display:block; margin:0 0 0.25rem 0; font-size:0.9rem; color:#c7d0dd; }}\
         input {{ width:100%; box-sizing:border-box; border:1px solid #3a4455; border-radius:8px; padding:0.6rem 0.75rem; margin:0 0 0.75rem 0; background:#0f141c; color:#f5f7fb; }}\
         button {{ width:100%; border:0; border-radius:8px; padding:0.65rem 0.75rem; font-weight:600; background:#6ad490; color:#131417; cursor:pointer; }}\
         .error {{ margin:0 0 0.75rem 0; color:#ff8d8d; font-size:0.9rem; }}\
         .hint {{ margin:0.75rem 0 0; color:#a9b5c5; font-size:0.8rem; }}\
         a {{ color:#9bd0ff; }}\
         </style>\
         </head>\
         <body>\
         <main class=\"card\">\
         <h1>Create RaceHub account</h1>\
         {error_html}\
         <form method=\"post\" action=\"/register\">\
         <input type=\"hidden\" name=\"next\" value=\"{escaped_next}\" />\
         <label for=\"username\">Username</label>\
         <input id=\"username\" name=\"username\" autocomplete=\"username\" required value=\"{escaped_username}\" />\
         <label for=\"password\">Password</label>\
         <input id=\"password\" name=\"password\" type=\"password\" autocomplete=\"new-password\" required />\
         <button type=\"submit\">Create account</button>\
         </form>\
         <p class=\"hint\">Already have an account? <a href=\"/login?next={}\">Sign in</a></p>\
         </main>\
         </body>\
         </html>",
        urlencoding::encode(next)
    ))
}

fn load_index_html(state: &AppState) -> Result<String, ApiError> {
    let Some(static_dir) = &state.static_dir else {
        return Err(ApiError::not_found("static file serving is disabled"));
    };
    let index = static_dir.join("index.html");
    std::fs::read_to_string(&index).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::not_found("index.html was not found in static dir")
        } else {
            ApiError::internal(format!("failed to read '{}': {e}", index.display()))
        }
    })
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn bearer_token_opt(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn session_cookie_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in cookie_header.split(';') {
        let mut parts = pair.trim().splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next()?.trim();
        if key == COOKIE_NAME && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn session_cookie(token: &str, secure: bool) -> HeaderValue {
    let secure_part = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={token}; HttpOnly; Path=/; SameSite=Lax{secure_part}"
    ))
    .expect("valid session cookie")
}

fn expired_session_cookie(secure: bool) -> HeaderValue {
    let secure_part = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0{secure_part}"
    ))
    .expect("valid expired cookie")
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
            is_public INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY(owner_user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        ",
    )?;

    let mut has_is_public = false;
    let mut stmt = conn.prepare("PRAGMA table_info(artifacts)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == "is_public" {
            has_is_public = true;
            break;
        }
    }

    if !has_is_public {
        conn.execute(
            "ALTER TABLE artifacts ADD COLUMN is_public INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    Ok(())
}

#[allow(dead_code)]
fn _ensure_under(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use race_protocol::{
        ArtifactSummary, LoginResponse, UpdateArtifactVisibilityRequest, UploadArtifactRequest,
    };
    use tower::ServiceExt;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let mut rng = rand::rng();
        let suffix: u64 = rng.random();
        let dir = std::env::temp_dir().join(format!("{prefix}_{suffix}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn setup_test_state(
        auth_mode: AuthMode,
        registration_enabled: bool,
    ) -> (AppState, PathBuf, PathBuf) {
        let static_dir = unique_temp_dir("racehub_static");
        let artifacts_dir = unique_temp_dir("racehub_artifacts");
        std::fs::write(
            static_dir.join("index.html"),
            "<html><body>game entry</body></html>",
        )
        .expect("write index");

        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        run_migrations(&conn).expect("run migrations");
        ensure_local_user(&conn).expect("ensure local user");
        let state = AppState {
            db: Arc::new(Mutex::new(conn)),
            artifacts_dir: artifacts_dir.clone(),
            static_dir: Some(static_dir.clone()),
            auth_mode,
            cookie_secure: false,
            registration_enabled,
        };
        (state, static_dir, artifacts_dir)
    }

    async fn create_user(state: &AppState, username: &str, password: &str) {
        let hash = hash_password(password).expect("hash password");
        let db = state.db.lock().await;
        db.execute(
            "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
            params![username, hash, now_utc()],
        )
        .expect("insert user");
    }

    async fn make_session_cookie(state: &AppState, username: &str, password: &str) -> String {
        let (_, token) = create_session_for_credentials(state, username, password)
            .await
            .expect("create session");
        format!("{COOKIE_NAME}={token}")
    }

    async fn upload_artifact_with_cookie(
        app: &Router,
        cookie: &str,
        name: &str,
    ) -> (StatusCode, i64) {
        let payload = UploadArtifactRequest {
            name: name.to_string(),
            note: None,
            target: "riscv32imafc-unknown-none-elf".to_string(),
            elf_base64: base64::engine::general_purpose::STANDARD.encode([0x7f, b'E', b'L', b'F']),
        };
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/artifacts")
                    .header(header::COOKIE, cookie)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&payload).expect("serialize payload"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = resp.status();
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let parsed: UploadArtifactResponse = serde_json::from_slice(&body).expect("upload json");
        (status, parsed.artifact_id)
    }

    async fn list_artifacts_with_cookie(app: &Router, cookie: &str) -> Vec<ArtifactSummary> {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/artifacts")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        serde_json::from_slice(&body).expect("artifact list json")
    }

    async fn update_visibility_with_cookie(
        app: &Router,
        cookie: &str,
        artifact_id: i64,
        is_public: bool,
    ) -> StatusCode {
        let payload = UpdateArtifactVisibilityRequest { is_public };
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/api/v1/artifacts/{artifact_id}/visibility"))
                    .header(header::COOKIE, cookie)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&payload).expect("serialize payload"),
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        resp.status()
    }

    async fn download_artifact_with_cookie(
        app: &Router,
        cookie: &str,
        artifact_id: i64,
    ) -> StatusCode {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/artifacts/{artifact_id}"))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        resp.status()
    }

    #[tokio::test]
    async fn unauthenticated_game_entry_shows_login_in_required_mode() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        let app = build_app(state, Some(static_dir.clone()));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let text = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(text.contains("Sign in to RaceHub"));
        assert!(text.contains("name=\"next\" value=\"/\""));

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn authenticated_game_entry_serves_index() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        let cookie = make_session_cookie(&state, "alice", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let text = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(text.contains("game entry"));

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn web_login_success_sets_cookie_and_redirects() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "username=alice&password=password123&next=%2Findex.html",
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            resp.headers()
                .get(header::LOCATION)
                .expect("location")
                .to_str()
                .expect("location str"),
            "/index.html"
        );
        let cookie = resp
            .headers()
            .get(header::SET_COOKIE)
            .expect("set-cookie")
            .to_str()
            .expect("cookie str");
        assert!(cookie.contains("racehub_session="));

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn web_login_failure_renders_error() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "username=alice&password=wrongpassword&next=%2Findex.html",
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert!(resp.headers().get(header::SET_COOKIE).is_none());
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let text = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(text.contains("Invalid username or password"));

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[test]
    fn sanitize_next_rejects_external_targets() {
        assert_eq!(sanitize_next("https://evil.com"), "/");
        assert_eq!(sanitize_next("//evil.com"), "/");
        assert_eq!(sanitize_next("/assets/foo"), "/assets/foo");
    }

    #[tokio::test]
    async fn api_login_still_returns_json_and_cookie() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        "{\"username\":\"alice\",\"password\":\"password123\"}",
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().contains_key(header::SET_COOKIE));
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let parsed: LoginResponse = serde_json::from_slice(&body).expect("login response json");
        assert_eq!(parsed.user.username, "alice");
        assert!(!parsed.token.is_empty());

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn capabilities_include_registration_enabled() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, false);
        let app = build_app(state, Some(static_dir.clone()));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/capabilities")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let caps: ServerCapabilities = serde_json::from_slice(&body).expect("caps json");
        assert!(caps.auth_required);
        assert!(!caps.registration_enabled);

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn api_register_blocked_when_registration_disabled() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, false);
        let app = build_app(state, Some(static_dir.clone()));
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        "{\"username\":\"alice\",\"password\":\"password123\"}",
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let error: ErrorResponse = serde_json::from_slice(&body).expect("error json");
        assert_eq!(error.error, "registration is disabled");

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn register_flow_creates_session_and_redirects() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        let app = build_app(state, Some(static_dir.clone()));
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/register")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "username=bob&password=password123&next=%2Findex.html",
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            resp.headers()
                .get(header::LOCATION)
                .expect("location")
                .to_str()
                .expect("location str"),
            "/index.html"
        );
        assert!(resp.headers().contains_key(header::SET_COOKIE));

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn register_routes_redirect_when_disabled() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, false);
        let app = build_app(state, Some(static_dir.clone()));
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/register?next=%2Findex.html")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            resp.headers()
                .get(header::LOCATION)
                .expect("location")
                .to_str()
                .expect("location str"),
            "/login?next=%2Findex.html&error=registration_disabled"
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/register")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(
                        "username=bob&password=password123&next=%2Findex.html",
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            resp.headers()
                .get(header::LOCATION)
                .expect("location")
                .to_str()
                .expect("location str"),
            "/login?next=%2Findex.html&error=registration_disabled"
        );

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn login_page_hides_register_link_when_disabled() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, false);
        let app = build_app(state, Some(static_dir.clone()));
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/login?next=%2F")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let text = String::from_utf8(body.to_vec()).expect("utf8");
        assert!(!text.contains("Create account"));

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn uploads_are_private_by_default() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        let alice_cookie = make_session_cookie(&state, "alice", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let (status, artifact_id) = upload_artifact_with_cookie(&app, &alice_cookie, "a.elf").await;
        assert_eq!(status, StatusCode::OK);

        let artifacts = list_artifacts_with_cookie(&app, &alice_cookie).await;
        let artifact = artifacts
            .into_iter()
            .find(|a| a.id == artifact_id)
            .expect("artifact exists");
        assert!(!artifact.is_public);
        assert!(artifact.owned_by_me);

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn list_artifacts_in_required_mode_shows_own_and_public_others() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        create_user(&state, "bob", "password123").await;
        let alice_cookie = make_session_cookie(&state, "alice", "password123").await;
        let bob_cookie = make_session_cookie(&state, "bob", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let (_, alice_artifact_id) =
            upload_artifact_with_cookie(&app, &alice_cookie, "alice.elf").await;
        let (_, bob_artifact_id) = upload_artifact_with_cookie(&app, &bob_cookie, "bob.elf").await;
        assert_eq!(
            update_visibility_with_cookie(&app, &bob_cookie, bob_artifact_id, true).await,
            StatusCode::NO_CONTENT
        );

        let alice_view = list_artifacts_with_cookie(&app, &alice_cookie).await;
        assert!(
            alice_view
                .iter()
                .any(|a| a.id == alice_artifact_id && a.owned_by_me)
        );
        assert!(
            alice_view
                .iter()
                .any(|a| a.id == bob_artifact_id && !a.owned_by_me && a.is_public)
        );

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn list_artifacts_hides_private_others() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        create_user(&state, "bob", "password123").await;
        let alice_cookie = make_session_cookie(&state, "alice", "password123").await;
        let bob_cookie = make_session_cookie(&state, "bob", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let (_, bob_artifact_id) = upload_artifact_with_cookie(&app, &bob_cookie, "bob.elf").await;
        let alice_view = list_artifacts_with_cookie(&app, &alice_cookie).await;
        assert!(!alice_view.iter().any(|a| a.id == bob_artifact_id));

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn download_public_artifact_allowed_for_non_owner() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        create_user(&state, "bob", "password123").await;
        let alice_cookie = make_session_cookie(&state, "alice", "password123").await;
        let bob_cookie = make_session_cookie(&state, "bob", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let (_, artifact_id) = upload_artifact_with_cookie(&app, &bob_cookie, "bob.elf").await;
        assert_eq!(
            update_visibility_with_cookie(&app, &bob_cookie, artifact_id, true).await,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            download_artifact_with_cookie(&app, &alice_cookie, artifact_id).await,
            StatusCode::OK
        );

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn download_private_artifact_denied_for_non_owner() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        create_user(&state, "bob", "password123").await;
        let alice_cookie = make_session_cookie(&state, "alice", "password123").await;
        let bob_cookie = make_session_cookie(&state, "bob", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let (_, artifact_id) = upload_artifact_with_cookie(&app, &bob_cookie, "bob.elf").await;
        assert_eq!(
            download_artifact_with_cookie(&app, &alice_cookie, artifact_id).await,
            StatusCode::UNAUTHORIZED
        );

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn owner_can_toggle_visibility() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        let alice_cookie = make_session_cookie(&state, "alice", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let (_, artifact_id) = upload_artifact_with_cookie(&app, &alice_cookie, "alice.elf").await;
        assert_eq!(
            update_visibility_with_cookie(&app, &alice_cookie, artifact_id, true).await,
            StatusCode::NO_CONTENT
        );

        let artifacts = list_artifacts_with_cookie(&app, &alice_cookie).await;
        let artifact = artifacts
            .into_iter()
            .find(|a| a.id == artifact_id)
            .expect("artifact exists");
        assert!(artifact.is_public);

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[tokio::test]
    async fn non_owner_cannot_toggle_visibility() {
        let (state, static_dir, artifacts_dir) = setup_test_state(AuthMode::Required, true);
        create_user(&state, "alice", "password123").await;
        create_user(&state, "bob", "password123").await;
        let alice_cookie = make_session_cookie(&state, "alice", "password123").await;
        let bob_cookie = make_session_cookie(&state, "bob", "password123").await;
        let app = build_app(state, Some(static_dir.clone()));

        let (_, artifact_id) = upload_artifact_with_cookie(&app, &bob_cookie, "bob.elf").await;
        assert_eq!(
            update_visibility_with_cookie(&app, &alice_cookie, artifact_id, true).await,
            StatusCode::UNAUTHORIZED
        );

        let _ = std::fs::remove_dir_all(static_dir);
        let _ = std::fs::remove_dir_all(artifacts_dir);
    }

    #[test]
    fn migration_adds_is_public_column() {
        let conn = Connection::open_in_memory().expect("open in-memory sqlite");
        conn.execute_batch(
            "
            CREATE TABLE artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner_user_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                note TEXT,
                target TEXT NOT NULL,
                elf_path TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            ",
        )
        .expect("create legacy artifacts table");

        run_migrations(&conn).expect("run migrations");

        let mut stmt = conn
            .prepare("PRAGMA table_info(artifacts)")
            .expect("prepare pragma");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query columns");
        let mut has_is_public = false;
        for row in rows {
            if row.expect("column") == "is_public" {
                has_is_public = true;
                break;
            }
        }
        assert!(has_is_public);
    }
}
