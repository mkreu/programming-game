use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub auth_required: bool,
    pub mode: String,
    #[serde(default = "default_registration_enabled")]
    pub registration_enabled: bool,
}

fn default_registration_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub id: i64,
    pub owner_user_id: i64,
    pub owner_username: String,
    pub name: String,
    pub note: Option<String>,
    pub target: String,
    pub is_public: bool,
    pub owned_by_me: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadArtifactRequest {
    pub name: String,
    pub note: Option<String>,
    pub target: String,
    pub elf_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadArtifactResponse {
    pub artifact_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateArtifactVisibilityRequest {
    pub is_public: bool,
}
