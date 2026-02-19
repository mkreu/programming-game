use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSummary {
    pub id: i64,
    pub name: String,
    pub language: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScriptRequest {
    pub name: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScriptVersionRequest {
    pub commit_hash: Option<String>,
    pub source_bundle_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptVersionSummary {
    pub id: i64,
    pub script_id: i64,
    pub version: i64,
    pub commit_hash: Option<String>,
    pub source_bundle_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadArtifactRequest {
    pub script_version_id: i64,
    pub target: String,
    pub elf_base64: String,
    pub build_meta_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadArtifactResponse {
    pub artifact_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub id: i64,
    pub script_version_id: i64,
    pub target: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRaceRecordRequest {
    pub track_id: String,
    pub result_json: String,
    pub replay_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceRecordSummary {
    pub id: i64,
    pub user_id: i64,
    pub track_id: String,
    pub result_json: String,
    pub replay_json: Option<String>,
    pub created_at: String,
}
