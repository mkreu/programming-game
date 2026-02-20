#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::{
    collections::HashMap,
    f32::consts::PI,
    sync::{Arc, Mutex},
};

use avian2d::prelude::{forces::ForcesItem, *};
use base64::Engine;
use bevy::{
    color::palettes::css::{GREEN, RED, WHITE, YELLOW},
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
};
use emulator::bevy::{CpuComponent, cpu_system};
use emulator::cpu::LogDevice;
use race_protocol::{
    ArtifactSummary, ServerCapabilities, UploadArtifactRequest, UploadArtifactResponse,
};
#[cfg(not(target_arch = "wasm32"))]
use race_protocol::{LoginRequest, LoginResponse};
#[cfg(not(target_arch = "wasm32"))]
use racehub::{AuthMode, ServerConfig};

use racing::Car;
use racing::devices::{self, TrackRadarBorders};
use racing::devices::{
    CarControlsDevice, CarRadarDevice, CarStateDevice, SplineDevice, TrackRadarDevice,
};
use racing::track;
use racing::track_format::TrackFile;

mod ui;

// Re-export types used by the UI module.
pub(crate) use main_game::*;

/// All game-specific types live here so `ui` can import them via `crate::main_game::*`.
mod main_game {
    use super::*;

    // ── Simulation state ────────────────────────────────────────────────

    #[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub enum SimState {
        #[default]
        PreRace,
        Racing,
        Paused,
    }

    // ── Events ──────────────────────────────────────────────────────────

    #[derive(Message)]
    pub struct SpawnCarRequest {
        pub driver: DriverType,
    }

    #[derive(Message)]
    pub enum WebApiCommand {
        RefreshCapabilities,
        LoadArtifacts,
        UploadArtifact,
    }

    // ── Resources ───────────────────────────────────────────────────────

    #[derive(Resource)]
    pub struct RaceManager {
        pub cars: Vec<CarEntry>,
        pub selected_driver: Option<DriverType>,
        pub next_car_id: u32,
    }

    impl Default for RaceManager {
        fn default() -> Self {
            Self {
                cars: Vec::new(),
                selected_driver: None,
                next_car_id: 1,
            }
        }
    }

    pub struct CarEntry {
        pub entity: Entity,
        pub name: String,
        pub driver: DriverType,
        pub console_output: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum DriverType {
        RemoteArtifact { id: i64 },
    }

    impl DriverType {
        pub fn label(&self) -> String {
            match self {
                DriverType::RemoteArtifact { id } => format!("Artifact: #{id}"),
            }
        }
    }

    pub struct CompileResult {
        pub id: u64,
        pub binary: String,
        pub result: Result<Vec<u8>, String>,
    }

    #[derive(Resource)]
    pub struct ArtifactFetchPipeline {
        pub async_results: Arc<Mutex<Vec<CompileResult>>>,
        pub pending: HashMap<u64, DriverType>,
        pub next_request_id: u64,
        pub status_message: Option<String>,
    }

    #[derive(Resource, Default)]
    pub struct FollowCar {
        pub target: Option<Entity>,
    }

    #[derive(Debug, Clone)]
    pub enum WebApiEvent {
        Capabilities(Result<ServerCapabilities, String>),
        #[cfg(not(target_arch = "wasm32"))]
        Login(Result<LoginResponse, String>),
        Artifacts(Result<Vec<ArtifactSummary>, String>),
        UploadResult(Result<UploadArtifactResponse, String>),
    }

    #[derive(Resource, Clone)]
    pub struct WebApiQueue {
        pub events: Arc<Mutex<Vec<WebApiEvent>>>,
    }

    impl Default for WebApiQueue {
        fn default() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[derive(Resource)]
    pub struct WebPortalState {
        pub server_url: String,
        pub standalone_mode: bool,
        pub auth_required: Option<bool>,
        #[cfg(not(target_arch = "wasm32"))]
        pub token: Option<String>,
        #[cfg(not(target_arch = "wasm32"))]
        pub cli_credentials: Option<(String, String)>,
        pub artifacts: Vec<ArtifactSummary>,
        pub status_message: Option<String>,
    }

    impl Default for WebPortalState {
        fn default() -> Self {
            Self {
                server_url: std::env::var("RACEHUB_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8787".to_string()),
                standalone_mode: false,
                auth_required: None,
                #[cfg(not(target_arch = "wasm32"))]
                token: None,
                #[cfg(not(target_arch = "wasm32"))]
                cli_credentials: None,
                artifacts: Vec::new(),
                status_message: None,
            }
        }
    }

    // ── Components ──────────────────────────────────────────────────────

    #[derive(Component)]
    pub struct CarLabel {
        pub name: String,
    }

    /// Marker: when present on a car entity, debug gizmos are drawn for it.
    #[derive(Component)]
    pub struct DebugGizmos;

    // ── System Sets ─────────────────────────────────────────────────────
    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    pub enum CpuSystems {
        PreCpu,
        Cpu,
        PostCpu,
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    let mut standalone_mode = false;
    let mut track_path = "racing/assets/track1.toml".to_string();
    for arg in std::env::args().skip(1) {
        #[cfg(not(target_arch = "wasm32"))]
        if arg == "--standalone" {
            standalone_mode = true;
        } else {
            track_path = arg;
        }
        #[cfg(target_arch = "wasm32")]
        {
            track_path = arg;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    let standalone_url = if standalone_mode {
        let bind = std::env::var("RACEHUB_STANDALONE_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8787".to_string());
        spawn_embedded_racehub(bind.clone());
        Some(format!("http://{bind}"))
    } else {
        None
    };
    #[cfg(target_arch = "wasm32")]
    let standalone_url: Option<String> = None;

    let mut web_state = WebPortalState::default();
    if let Some(url) = standalone_url {
        web_state.server_url = url;
        web_state.standalone_mode = true;
        web_state.status_message = Some("Standalone mode: auth disabled".to_string());
    }
    #[cfg(not(target_arch = "wasm32"))]
    if !web_state.standalone_mode {
        match prompt_cli_credentials() {
            Ok(Some((username, password))) => {
                web_state.cli_credentials = Some((username.clone(), password));
                web_state.status_message = Some(format!("Using CLI credentials for '{username}'"));
            }
            Ok(None) => {
                web_state.status_message =
                    Some("No CLI credentials provided; remote auth may fail".to_string());
            }
            Err(error) => {
                web_state.status_message = Some(format!("CLI login prompt failed: {error}"));
            }
        }
    }

    App::new()
        .add_plugins((
            DefaultPlugins,
            FrameTimeDiagnosticsPlugin::default(),
            PhysicsPlugins::default(),
            ui::RaceUiPlugin,
        ))
        .init_state::<SimState>()
        .add_message::<SpawnCarRequest>()
        .add_message::<WebApiCommand>()
        .insert_resource(Gravity::ZERO)
        .insert_resource(Time::<Fixed>::from_duration(
            std::time::Duration::from_secs_f32(1.0 / 200.0),
        ))
        .insert_resource(TrackPath(track_path))
        .insert_resource(create_artifact_fetch_pipeline())
        .insert_resource(web_state)
        .insert_resource(WebApiQueue::default())
        .insert_resource(RaceManager::default())
        .insert_resource(FollowCar::default())
        .add_systems(
            Startup,
            (
                setup_track,
                setup.after(setup_track),
                trigger_initial_capability_check,
            ),
        )
        .add_systems(Startup, set_default_zoom.after(setup))
        // Pause/unpause avian2d physics based on SimState
        .add_systems(Startup, pause_physics)
        .add_systems(OnEnter(SimState::Racing), unpause_physics)
        .add_systems(OnEnter(SimState::Paused), pause_physics)
        .add_systems(OnEnter(SimState::PreRace), pause_physics)
        // Spawning: always active so cars can be added in PreRace
        .add_systems(
            Update,
            (
                handle_web_api_commands,
                process_web_api_events,
                handle_spawn_car_event,
                process_artifact_fetch_results,
            ),
        )
        // Keyboard driving: always active (only affects non-AI, non-emulator cars)
        .add_systems(Update, handle_car_input)
        .configure_sets(
            FixedUpdate,
            (CpuSystems::PreCpu, CpuSystems::Cpu, CpuSystems::PostCpu).chain(),
        )
        // emulator AI: only run while Racing
        .add_systems(
            FixedUpdate,
            (
                devices::car_state_system.in_set(CpuSystems::PreCpu),
                devices::car_radar_system.in_set(CpuSystems::PreCpu),
                devices::track_radar_system.in_set(CpuSystems::PreCpu),
                cpu_system::<RacingCpuConfig>.in_set(CpuSystems::Cpu),
                devices::car_controls_system.in_set(CpuSystems::PostCpu),
            )
                .run_if(in_state(SimState::Racing)),
        )
        // Physics forces: only while Racing
        .add_systems(
            FixedUpdate,
            apply_car_forces.run_if(in_state(SimState::Racing)),
        )
        .add_systems(Update, (update_fps_counter, update_camera, draw_gizmos))
        .run();
}

#[cfg(not(target_arch = "wasm32"))]
fn prompt_cli_credentials() -> Result<Option<(String, String)>, String> {
    use std::io::{self, Write};

    print!("RaceHub username (leave empty to skip): ");
    io::stdout()
        .flush()
        .map_err(|e| format!("stdout flush failed: {e}"))?;
    let mut username = String::new();
    io::stdin()
        .read_line(&mut username)
        .map_err(|e| format!("failed to read username: {e}"))?;
    let username = username.trim().to_string();
    if username.is_empty() {
        return Ok(None);
    }

    print!("RaceHub password: ");
    io::stdout()
        .flush()
        .map_err(|e| format!("stdout flush failed: {e}"))?;
    let mut password = String::new();
    io::stdin()
        .read_line(&mut password)
        .map_err(|e| format!("failed to read password: {e}"))?;
    let password = password.trim_end().to_string();
    if password.is_empty() {
        return Ok(None);
    }

    Ok(Some((username, password)))
}

#[derive(Resource)]
struct TrackPath(String);

const WHEEL_BASE: f32 = 1.18;
const WHEEL_TRACK: f32 = 0.95;

fn create_artifact_fetch_pipeline() -> ArtifactFetchPipeline {
    ArtifactFetchPipeline {
        async_results: Arc::new(Mutex::new(Vec::<CompileResult>::new())),
        pending: HashMap::new(),
        next_request_id: 1,
        status_message: None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_embedded_racehub(bind: String) {
    let mut config = ServerConfig::default();
    config.bind = bind;
    config.auth_mode = AuthMode::Disabled;
    config.db_path = PathBuf::from(
        std::env::var("RACEHUB_DB_PATH").unwrap_or_else(|_| "racehub.db".to_string()),
    );
    config.artifacts_dir = PathBuf::from(
        std::env::var("RACEHUB_ARTIFACTS_DIR").unwrap_or_else(|_| "racehub_artifacts".to_string()),
    );
    config.static_dir = None;

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime for embedded racehub");
        runtime
            .block_on(racehub::run_server(config))
            .expect("embedded racehub crashed");
    });
}

fn trigger_initial_capability_check(mut cmds: MessageWriter<WebApiCommand>) {
    cmds.write(WebApiCommand::RefreshCapabilities);
}

fn web_api_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

fn web_request_with_auth(url: String, _token: Option<&str>) -> ehttp::Request {
    let mut req = ehttp::Request::get(url);
    #[cfg(not(target_arch = "wasm32"))]
    let token = _token;
    #[cfg(target_arch = "wasm32")]
    let token: Option<&str> = None;
    if let Some(token) = token {
        req.headers
            .insert("Authorization", format!("Bearer {token}"));
    }
    req
}

fn push_web_event(queue: &Arc<Mutex<Vec<WebApiEvent>>>, event: WebApiEvent) {
    if let Ok(mut events) = queue.lock() {
        events.push(event);
    }
}

fn response_error(resp: &ehttp::Response) -> String {
    let body = String::from_utf8_lossy(&resp.bytes);
    format!("HTTP {} {}: {}", resp.status, resp.status_text, body.trim())
}

#[cfg(not(target_arch = "wasm32"))]
fn web_fetch_login(
    server_url: &str,
    username: &str,
    password: &str,
    queue: Arc<Mutex<Vec<WebApiEvent>>>,
) {
    let url = web_api_url(server_url, "/api/v1/auth/login");
    let request = match ehttp::Request::json(
        url,
        &LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    ) {
        Ok(req) => req,
        Err(err) => {
            push_web_event(
                &queue,
                WebApiEvent::Login(Err(format!("failed to serialize login request: {err}"))),
            );
            return;
        }
    };

    ehttp::fetch(request, move |result| {
        let event = match result {
            Ok(resp) if resp.ok => WebApiEvent::Login(
                resp.json::<LoginResponse>()
                    .map_err(|err| format!("invalid login response: {err}")),
            ),
            Ok(resp) => WebApiEvent::Login(Err(response_error(&resp))),
            Err(err) => WebApiEvent::Login(Err(format!("network error: {err}"))),
        };
        push_web_event(&queue, event);
    });
}

fn web_fetch_capabilities(server_url: &str, queue: Arc<Mutex<Vec<WebApiEvent>>>) {
    let url = web_api_url(server_url, "/api/v1/capabilities");
    let request = ehttp::Request::get(url);
    ehttp::fetch(request, move |result| {
        let event = match result {
            Ok(resp) if resp.ok => WebApiEvent::Capabilities(
                resp.json::<ServerCapabilities>()
                    .map_err(|err| format!("invalid capabilities response: {err}")),
            ),
            Ok(resp) => WebApiEvent::Capabilities(Err(response_error(&resp))),
            Err(err) => WebApiEvent::Capabilities(Err(format!("network error: {err}"))),
        };
        push_web_event(&queue, event);
    });
}

fn web_fetch_artifacts(server_url: &str, token: Option<&str>, queue: Arc<Mutex<Vec<WebApiEvent>>>) {
    let url = web_api_url(server_url, "/api/v1/artifacts");
    let request = web_request_with_auth(url, token);
    ehttp::fetch(request, move |result| {
        let event = match result {
            Ok(resp) if resp.ok => WebApiEvent::Artifacts(
                resp.json::<Vec<ArtifactSummary>>()
                    .map_err(|err| format!("invalid artifacts response: {err}")),
            ),
            Ok(resp) => WebApiEvent::Artifacts(Err(response_error(&resp))),
            Err(err) => WebApiEvent::Artifacts(Err(format!("network error: {err}"))),
        };
        push_web_event(&queue, event);
    });
}

fn web_upload_artifact(
    server_url: &str,
    _token: Option<&str>,
    name: String,
    note: Option<String>,
    elf: Vec<u8>,
    queue: Arc<Mutex<Vec<WebApiEvent>>>,
) {
    let url = web_api_url(server_url, "/api/v1/artifacts");
    let mut request = match ehttp::Request::json(
        url,
        &UploadArtifactRequest {
            name,
            note,
            target: "riscv32imafc-unknown-none-elf".to_string(),
            elf_base64: base64::engine::general_purpose::STANDARD.encode(elf),
        },
    ) {
        Ok(req) => req,
        Err(err) => {
            push_web_event(
                &queue,
                WebApiEvent::UploadResult(Err(format!(
                    "failed to serialize upload payload: {err}"
                ))),
            );
            return;
        }
    };
    request.method = "POST".to_string();
    #[cfg(not(target_arch = "wasm32"))]
    let token = _token;
    #[cfg(target_arch = "wasm32")]
    let token: Option<&str> = None;
    if let Some(token) = token {
        request
            .headers
            .insert("Authorization", format!("Bearer {token}"));
    }

    ehttp::fetch(request, move |result| {
        let event = match result {
            Ok(resp) if resp.ok => WebApiEvent::UploadResult(
                resp.json::<UploadArtifactResponse>()
                    .map_err(|err| format!("invalid upload response: {err}")),
            ),
            Ok(resp) => WebApiEvent::UploadResult(Err(response_error(&resp))),
            Err(err) => WebApiEvent::UploadResult(Err(format!("network error: {err}"))),
        };
        push_web_event(&queue, event);
    });
}

fn web_fetch_artifact_elf(
    server_url: &str,
    token: Option<&str>,
    artifact_id: i64,
    request_id: u64,
    results_queue: Arc<Mutex<Vec<CompileResult>>>,
) {
    let url = web_api_url(server_url, &format!("/api/v1/artifacts/{artifact_id}"));
    let request = web_request_with_auth(url, token);
    ehttp::fetch(request, move |result| {
        let compile_result = match result {
            Ok(resp) if resp.ok => CompileResult {
                id: request_id,
                binary: format!("artifact_{artifact_id}"),
                result: Ok(resp.bytes),
            },
            Ok(resp) => CompileResult {
                id: request_id,
                binary: format!("artifact_{artifact_id}"),
                result: Err(response_error(&resp)),
            },
            Err(err) => CompileResult {
                id: request_id,
                binary: format!("artifact_{artifact_id}"),
                result: Err(format!("network error: {err}")),
            },
        };
        if let Ok(mut pending) = results_queue.lock() {
            pending.push(compile_result);
        }
    });
}

fn maybe_auth_token(web_state: &WebPortalState) -> Result<Option<String>, String> {
    match web_state.auth_required {
        Some(true) => {
            #[cfg(target_arch = "wasm32")]
            {
                Ok(None)
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                web_state
                    .token
                    .clone()
                    .map(Some)
                    .ok_or_else(|| "Login required".to_string())
            }
        }
        Some(false) => Ok(None),
        None => Err("Server capabilities not loaded yet".to_string()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_artifact_for_upload_native() -> Result<Option<(String, Vec<u8>)>, String> {
    let Some(path) = rfd::FileDialog::new().pick_file() else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("failed to read file: {e}"))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact.elf".to_string());
    Ok(Some((name, bytes)))
}

#[cfg(target_arch = "wasm32")]
fn pick_artifact_for_upload_web(
    server_url: String,
    token: Option<String>,
    queue: Arc<Mutex<Vec<WebApiEvent>>>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(file) = rfd::AsyncFileDialog::new().pick_file().await else {
            return;
        };
        let bytes = file.read().await;
        let name = file.file_name();
        web_upload_artifact(&server_url, token.as_deref(), name, None, bytes, queue);
    });
}

fn handle_web_api_commands(
    mut commands: MessageReader<WebApiCommand>,
    mut web_state: ResMut<WebPortalState>,
    web_queue: Res<WebApiQueue>,
) {
    for command in commands.read() {
        match command {
            WebApiCommand::RefreshCapabilities => {
                web_state.status_message = Some("Loading server capabilities...".to_string());
                web_fetch_capabilities(&web_state.server_url, web_queue.events.clone());
            }
            WebApiCommand::LoadArtifacts => {
                if web_state.auth_required.is_none() {
                    web_state.status_message =
                        Some("Checking server capabilities first...".to_string());
                    web_fetch_capabilities(&web_state.server_url, web_queue.events.clone());
                    continue;
                }
                let token = match maybe_auth_token(&web_state) {
                    Ok(token) => token,
                    Err(error) => {
                        web_state.status_message = Some(error);
                        continue;
                    }
                };
                web_state.status_message = Some("Loading artifacts...".to_string());
                web_fetch_artifacts(
                    &web_state.server_url,
                    token.as_deref(),
                    web_queue.events.clone(),
                );
            }
            WebApiCommand::UploadArtifact => {
                if web_state.auth_required.is_none() {
                    web_state.status_message =
                        Some("Checking server capabilities first...".to_string());
                    web_fetch_capabilities(&web_state.server_url, web_queue.events.clone());
                    continue;
                }
                let token = match maybe_auth_token(&web_state) {
                    Ok(token) => token,
                    Err(error) => {
                        web_state.status_message = Some(error);
                        continue;
                    }
                };
                #[cfg(not(target_arch = "wasm32"))]
                match pick_artifact_for_upload_native() {
                    Ok(Some((name, bytes))) => {
                        web_state.status_message = Some(format!("Uploading '{name}'..."));
                        web_upload_artifact(
                            &web_state.server_url,
                            token.as_deref(),
                            name,
                            None,
                            bytes,
                            web_queue.events.clone(),
                        );
                    }
                    Ok(None) => {}
                    Err(error) => {
                        web_state.status_message = Some(error);
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    web_state.status_message = Some("Pick artifact to upload...".to_string());
                    pick_artifact_for_upload_web(
                        web_state.server_url.clone(),
                        token,
                        web_queue.events.clone(),
                    );
                }
            }
        }
    }
}

fn process_web_api_events(mut web_state: ResMut<WebPortalState>, web_queue: Res<WebApiQueue>) {
    let mut events = Vec::new();
    if let Ok(mut queue) = web_queue.events.lock() {
        events.append(&mut *queue);
    }

    for event in events {
        match event {
            WebApiEvent::Capabilities(result) => match result {
                Ok(caps) => {
                    web_state.auth_required = Some(caps.auth_required);
                    web_state.status_message = Some(format!(
                        "Connected: mode={}, auth_required={}",
                        caps.mode, caps.auth_required
                    ));
                    #[cfg(not(target_arch = "wasm32"))]
                    if caps.auth_required && web_state.token.is_none() {
                        if let Some((username, password)) = web_state.cli_credentials.clone() {
                            web_state.status_message =
                                Some(format!("Logging in as '{username}'..."));
                            web_fetch_login(
                                &web_state.server_url,
                                &username,
                                &password,
                                web_queue.events.clone(),
                            );
                            continue;
                        }
                    }
                    if let Ok(token) = maybe_auth_token(&web_state) {
                        web_fetch_artifacts(
                            &web_state.server_url,
                            token.as_deref(),
                            web_queue.events.clone(),
                        );
                    }
                }
                Err(error) => {
                    web_state.status_message = Some(format!("Capability check failed: {error}"));
                }
            },
            #[cfg(not(target_arch = "wasm32"))]
            WebApiEvent::Login(result) => match result {
                Ok(login) => {
                    web_state.token = Some(login.token);
                    web_state.status_message =
                        Some(format!("Logged in as {}", login.user.username));
                    web_fetch_artifacts(
                        &web_state.server_url,
                        web_state.token.as_deref(),
                        web_queue.events.clone(),
                    );
                }
                Err(error) => {
                    web_state.status_message = Some(format!("Login failed: {error}"));
                }
            },
            WebApiEvent::Artifacts(result) => match result {
                Ok(artifacts) => {
                    web_state.artifacts = artifacts;
                    web_state.status_message =
                        Some(format!("Loaded {} artifacts", web_state.artifacts.len()));
                }
                Err(error) => {
                    web_state.status_message = Some(format!("Loading artifacts failed: {error}"));
                }
            },
            WebApiEvent::UploadResult(result) => match result {
                Ok(upload) => {
                    web_state.status_message =
                        Some(format!("Uploaded artifact #{}", upload.artifact_id));
                    if let Ok(token) = maybe_auth_token(&web_state) {
                        web_fetch_artifacts(
                            &web_state.server_url,
                            token.as_deref(),
                            web_queue.events.clone(),
                        );
                    }
                }
                Err(error) => {
                    web_state.status_message = Some(format!("Upload failed: {error}"));
                }
            },
        }
    }
}

fn setup_track(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    track_path: Res<TrackPath>,
) {
    let track_file = TrackFile::load(std::path::Path::new(&track_path.0))
        .unwrap_or_else(|_| panic!("Failed to load track file: {}", track_path.0));

    let control_points = track_file.control_points_vec2();
    let track_width = track_file.metadata.track_width;
    let kerb_width = track_file.metadata.kerb_width;

    // Green ground plane
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(800.0, 800.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.6, 0.2))),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));

    let spline = track::build_spline(&control_points);

    commands.insert_resource(track::TrackSpline {
        spline: spline.clone(),
    });
    let (inner_border, outer_border) = track::sample_track_borders(&spline, track_width, 1000);
    commands.insert_resource(TrackRadarBorders {
        inner: inner_border,
        outer: outer_border,
    });

    // Track surface
    let track_mesh = track::create_track_mesh(&spline, track_width, 1000);
    commands.spawn((
        Mesh2d(meshes.add(track_mesh)),
        MeshMaterial2d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Kerbs
    let (inner_kerb, outer_kerb) =
        track::create_kerb_meshes(&spline, track_width, kerb_width, 1000);
    commands.spawn((
        Mesh2d(meshes.add(inner_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
    ));
    commands.spawn((
        Mesh2d(meshes.add(outer_kerb)),
        MeshMaterial2d(materials.add(ColorMaterial::default())),
        Transform::from_xyz(0.0, 0.0, 0.1),
    ));
}

fn setup(mut commands: Commands) {
    // FPS counter
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Text::new("FPS: --"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(WHITE.into()),
        FpsCounterText,
    ));

    // Camera — starts free (not following any car)
    commands.spawn(Camera2d);
}

#[derive(Component)]
struct FpsCounterText;

fn update_fps_counter(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsCounterText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|value| value.smoothed())
    {
        text.0 = format!("FPS: {fps:>3.0}");
    }
}

fn set_default_zoom(mut camera_query: Query<&mut Projection, With<Camera2d>>) {
    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Orthographic(ref mut ortho) = *projection {
        ortho.scale = 0.05;
    }
}

fn pause_physics(mut physics_time: ResMut<Time<Physics>>) {
    physics_time.pause();
}

fn unpause_physics(mut physics_time: ResMut<Time<Physics>>) {
    physics_time.unpause();
}

// ── Starting grid positions ─────────────────────────────────────────────────

/// Return the staggered grid offset for the Nth car (0-indexed).
fn grid_offset(index: usize) -> Vec2 {
    let row = index as f32;
    let side = if index % 2 == 0 { 1.0 } else { -1.0 };
    Vec2::new(row * 2.0, side * 2.0)
}

// ── Car spawning via event ──────────────────────────────────────────────────

fn handle_spawn_car_event(
    mut events: MessageReader<SpawnCarRequest>,
    mut compile_pipeline: ResMut<ArtifactFetchPipeline>,
    web_state: Res<WebPortalState>,
    state: Res<State<SimState>>,
) {
    if *state.get() != SimState::PreRace {
        return;
    }

    for event in events.read() {
        let request_id = compile_pipeline.next_request_id;
        compile_pipeline.next_request_id += 1;
        compile_pipeline
            .pending
            .insert(request_id, event.driver.clone());

        match &event.driver {
            DriverType::RemoteArtifact { id } => {
                let token = match maybe_auth_token(&web_state) {
                    Ok(token) => token,
                    Err(error) => {
                        compile_pipeline.pending.remove(&request_id);
                        compile_pipeline.status_message = Some(error);
                        continue;
                    }
                };
                compile_pipeline.status_message = Some(format!("Downloading artifact #{id}..."));
                web_fetch_artifact_elf(
                    &web_state.server_url,
                    token.as_deref(),
                    *id,
                    request_id,
                    compile_pipeline.async_results.clone(),
                );
            }
        }
    }
}

fn process_artifact_fetch_results(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    track_path: Res<TrackPath>,
    track_spline: Res<track::TrackSpline>,
    mut manager: ResMut<RaceManager>,
    mut compile_pipeline: ResMut<ArtifactFetchPipeline>,
    state: Res<State<SimState>>,
) {
    let mut results = Vec::new();
    if let Ok(mut async_results) = compile_pipeline.async_results.lock() {
        results.append(&mut *async_results);
    }
    for result in results {
        let Some(driver) = compile_pipeline.pending.remove(&result.id) else {
            continue;
        };

        match result.result {
            Ok(elf_bytes) => {
                if *state.get() != SimState::PreRace {
                    compile_pipeline.status_message = Some(format!(
                        "Discarded compiled '{}' result (race already started)",
                        result.binary
                    ));
                    continue;
                }

                spawn_car_entry(
                    &mut commands,
                    &asset_server,
                    &track_path,
                    &track_spline,
                    &mut manager,
                    driver,
                    Some(elf_bytes),
                );
                compile_pipeline.status_message =
                    Some(format!("Loaded and spawned '{}'", result.binary));
            }
            Err(error) => {
                compile_pipeline.status_message = Some(format!(
                    "Artifact load failed for '{}': {}",
                    result.binary, error
                ));
            }
        }
    }
}

fn spawn_car_entry(
    commands: &mut Commands,
    asset_server: &AssetServer,
    track_path: &TrackPath,
    track_spline: &track::TrackSpline,
    manager: &mut RaceManager,
    driver: DriverType,
    elf_bytes: Option<Vec<u8>>,
) {
    let car_index = manager.cars.len();
    let offset = grid_offset(car_index);

    let track_file = TrackFile::load(std::path::Path::new(&track_path.0))
        .unwrap_or_else(|_| panic!("Failed to load track file: {}", track_path.0));
    let start_point = track::first_point_from_file(&track_file);

    let position = start_point + offset;
    let car_name = format!("Car {}", manager.next_car_id);
    let entity = spawn_car(
        commands,
        asset_server,
        position,
        track_spline,
        &car_name,
        elf_bytes.as_deref(),
    );
    manager.cars.push(CarEntry {
        entity,
        name: car_name,
        driver,
        console_output: String::new(),
    });
    manager.next_car_id += 1;
}

fn spawn_car(
    commands: &mut Commands,
    asset_server: &AssetServer,
    position: Vec2,
    track_spline: &track::TrackSpline,
    name: &str,
    bot_elf: Option<&[u8]>,
) -> Entity {
    let sprite_scale = Vec3::splat(0.008);

    let mut entity = commands.spawn((
        Transform::from_xyz(position.x, position.y, 1.0)
            .with_rotation(Quat::from_axis_angle(Vec3::Z, PI / 2.0)),
        Visibility::default(),
        RigidBody::Dynamic,
        LinearDamping(0.1),
        Friction::new(0.1),
        Restitution::new(0.2),
        Car {
            steer: 0.0,
            accelerator: 0.0,
            brake: 0.0,
        },
        CarLabel {
            name: name.to_string(),
        },
    ));

    let Some(bot_elf) = bot_elf else {
        panic!("Missing bot ELF bytes for emulator-driven car");
    };
    let cpu = CpuComponent::new(bot_elf, 10000);
    entity.insert((
        EmulatorDriver,
        cpu,
        LogDevice::default(),
        CarStateDevice::default(),
        CarControlsDevice::default(),
        SplineDevice::new(track_spline),
        TrackRadarDevice::default(),
        CarRadarDevice::default(),
    ));

    let entity_id = entity.id();

    entity.with_children(|parent| {
        parent.spawn((
            Collider::rectangle(1.25, 2.0),
            Transform::from_xyz(0.0, 0.66, 0.0),
        ));

        parent.spawn((
            Sprite::from_image(asset_server.load("kart.png")),
            Transform::from_xyz(0.0, 0.66, 0.1).with_scale(sprite_scale),
        ));

        // Front left wheel
        parent
            .spawn((
                Transform::from_xyz(-WHEEL_TRACK / 2.0, WHEEL_BASE, 0.1),
                Visibility::default(),
                FrontWheel,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Sprite::from_image(asset_server.load("kart_wheel.png")),
                    Transform::default()
                        .with_scale(sprite_scale)
                        .with_rotation(Quat::from_rotation_z(0.0)),
                ));
            });
        // Front right wheel
        parent
            .spawn((
                Transform::from_xyz(WHEEL_TRACK / 2.0, WHEEL_BASE, 0.1),
                Visibility::default(),
                FrontWheel,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Sprite::from_image(asset_server.load("kart_wheel.png")),
                    Transform::default()
                        .with_scale(sprite_scale)
                        .with_rotation(Quat::from_rotation_z(PI)),
                ));
            });
    });

    entity_id
}

#[derive(Component)]
struct EmulatorDriver;

#[derive(Component)]
struct FrontWheel;

emulator::define_cpu_config! {
    RacingCpuConfig {
        1 => LogDevice,
        2 => CarStateDevice,
        3 => CarControlsDevice,
        4 => SplineDevice,
        5 => TrackRadarDevice,
        6 => CarRadarDevice,
    }
}

fn handle_car_input(
    mut car_query: Query<&mut Car, Without<EmulatorDriver>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    for mut car in &mut car_query {
        car.accelerator = if keyboard.pressed(KeyCode::KeyW) {
            1.0
        } else {
            0.0
        };
        car.brake = if keyboard.pressed(KeyCode::KeyS) {
            1.0
        } else {
            0.0
        };

        let max_steer = PI / 6.0;
        let steer_rate = 0.05 * car.steer.abs().max(0.1);
        if keyboard.pressed(KeyCode::KeyA) {
            car.steer = (-max_steer).max(car.steer - steer_rate);
        } else if keyboard.pressed(KeyCode::KeyD) {
            car.steer = max_steer.min(car.steer + steer_rate);
        } else {
            car.steer = if car.steer > 0.0 {
                (car.steer - steer_rate).max(0.0)
            } else {
                (car.steer + steer_rate).min(0.0)
            };
        }
    }
}

fn apply_car_forces(
    mut car_query: Query<(
        Entity,
        &Transform,
        &mut Car,
        &Children,
        Forces,
        Has<DebugGizmos>,
    )>,
    mut wheel_query: Query<&mut Transform, (With<FrontWheel>, Without<Car>)>,
    mut gizmos: Gizmos,
) {
    for (_entity, transform, car, children, mut forces, show_gizmos) in &mut car_query {
        let acceleration = 30.0;
        let braking = 50.0;

        let position = transform.translation.xy();
        let forward = transform.up().xy().normalize();
        let left = forward.perp();

        if car.brake > 0.0 {
            forces.apply_linear_acceleration(forward * -braking * car.brake);
            if show_gizmos {
                gizmos.arrow_2d(
                    position,
                    position + forward * -braking * car.brake * 0.3,
                    WHITE,
                );
            }
        } else if car.accelerator > 0.0 {
            forces.apply_linear_acceleration(forward * acceleration * car.accelerator);
            if show_gizmos {
                gizmos.arrow_2d(
                    position,
                    position + forward * acceleration * car.accelerator * 0.3,
                    WHITE,
                );
            }
        }

        apply_wheel_force(
            position,
            forward * WHEEL_BASE + left * -WHEEL_TRACK / 2.0,
            Vec2::from_angle(-car.steer).rotate(forward),
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );
        apply_wheel_force(
            position,
            forward * WHEEL_BASE + left * WHEEL_TRACK / 2.0,
            Vec2::from_angle(-car.steer).rotate(forward),
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );
        apply_wheel_force(
            position,
            left * -WHEEL_TRACK / 2.0,
            forward,
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );
        apply_wheel_force(
            position,
            left * WHEEL_TRACK / 2.0,
            forward,
            &mut forces,
            &mut gizmos,
            show_gizmos,
        );

        for child in children.iter() {
            if let Ok(mut wheel_transform) = wheel_query.get_mut(child) {
                wheel_transform.rotation = Quat::from_rotation_z(-car.steer);
            }
        }
    }
}

fn apply_wheel_force(
    car_position: Vec2,
    wheel_offset: Vec2,
    wheel_forward: Vec2,
    forces: &mut ForcesItem<'_, '_>,
    gizmos: &mut Gizmos,
    show_gizmos: bool,
) {
    let wheel_pos = car_position + wheel_offset;
    let wheel_left = wheel_forward.perp();

    if show_gizmos {
        gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_forward * 1.0, YELLOW);
        gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_left * 0.5, YELLOW);
    }

    let o = forces.angular_velocity();
    let l = forces.linear_velocity();
    let wow = wheel_pos - car_position;
    let wheel_velocity = l + Vec2::new(-o * wow.y, o * wow.x);

    if show_gizmos {
        gizmos.arrow_2d(wheel_pos, wheel_pos + wheel_velocity * 0.1, GREEN);
    }

    if wheel_velocity.length() > 0.1 {
        let force = -wheel_velocity.normalize().dot(wheel_left)
            * wheel_left
            * 10.0_f32.min(wheel_velocity.length() * 5.0);
        if show_gizmos {
            gizmos.arrow_2d(wheel_pos, wheel_pos + force, RED);
        }
        forces.apply_linear_acceleration_at_point(force, wheel_pos);
    }
}

fn draw_gizmos(car_query: Query<(&Transform, &Car), With<DebugGizmos>>, mut gizmos: Gizmos) {
    for (transform, _car) in &car_query {
        gizmos.cross(transform.to_isometry(), 0.2, RED);
        gizmos.cross(
            Isometry3d::new(
                transform.translation + transform.up() * WHEEL_BASE,
                transform.rotation,
            ),
            0.2,
            RED,
        );
    }
}

fn update_camera(
    car_query: Query<&Transform, With<Car>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), (With<Camera2d>, Without<Car>)>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut motion_events: MessageReader<MouseMotion>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    follow: Res<FollowCar>,
) {
    let Ok((mut camera_transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    let mut current_scale = 0.05_f32;
    if let Projection::Orthographic(ref mut ortho) = *projection {
        for event in scroll_events.read() {
            let zoom_delta = match event.unit {
                bevy::input::mouse::MouseScrollUnit::Line => event.y * 0.1,
                bevy::input::mouse::MouseScrollUnit::Pixel => event.y * 0.001,
            };

            ortho.scale *= 1.0 - zoom_delta;
            ortho.scale = ortho.scale.clamp(0.001, 10.0);
        }
        current_scale = ortho.scale;
    }

    // If following a car, snap camera to it
    if let Some(follow_entity) = follow.target {
        if let Ok(car_tf) = car_query.get(follow_entity) {
            camera_transform.translation.x = car_tf.translation.x;
            camera_transform.translation.y = car_tf.translation.y;
            return; // Skip manual panning when following
        }
    }

    // Free camera: middle-mouse or right-mouse drag to pan
    if mouse_buttons.pressed(MouseButton::Middle) || mouse_buttons.pressed(MouseButton::Right) {
        for event in motion_events.read() {
            camera_transform.translation.x -= event.delta.x * current_scale;
            camera_transform.translation.y += event.delta.y * current_scale;
        }
    }
}
