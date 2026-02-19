use std::{path::PathBuf, process::Stdio};

use base64::Engine;
use clap::{Parser, Subcommand};
use race_protocol::{
    CreateScriptRequest, CreateScriptVersionRequest, LoginRequest, LoginResponse, ScriptSummary,
    UploadArtifactRequest, UploadArtifactResponse,
};
use serde::{Deserialize, Serialize};

const DEFAULT_TARGET: &str = "riscv32imafc-unknown-none-elf";

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:8787")]
    server: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Login {
        username: String,
        password: String,
    },
    Register {
        username: String,
        password: String,
    },
    ListScripts,
    CreateScript {
        name: String,
        #[arg(long, default_value = "rust")]
        language: String,
    },
    CreateVersion {
        script_id: i64,
        #[arg(long)]
        commit_hash: Option<String>,
        #[arg(long)]
        source_bundle_path: Option<String>,
    },
    UploadArtifact {
        script_version_id: i64,
        elf_path: PathBuf,
        #[arg(long, default_value = DEFAULT_TARGET)]
        target: String,
    },
    BuildAndUpload {
        script_version_id: i64,
        #[arg(long)]
        bot_dir: PathBuf,
        #[arg(long)]
        bin: String,
        #[arg(long, default_value = DEFAULT_TARGET)]
        target: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConnectorConfig {
    server: String,
    token: String,
    username: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let http = reqwest::Client::new();

    match cli.command {
        Commands::Login { username, password } => {
            let url = format!("{}/api/v1/auth/login", cli.server.trim_end_matches('/'));
            let response = http
                .post(url)
                .json(&LoginRequest {
                    username: username.clone(),
                    password,
                })
                .send()
                .await?;

            if !response.status().is_success() {
                let body = response.text().await?;
                return Err(format!("login failed: {body}").into());
            }

            let login: LoginResponse = response.json().await?;
            let config = ConnectorConfig {
                server: cli.server,
                token: login.token,
                username,
            };
            save_config(&config)?;
            println!("Logged in as {}", config.username);
        }
        Commands::Register { username, password } => {
            let url = format!("{}/api/v1/auth/register", cli.server.trim_end_matches('/'));
            let body = serde_json::json!({ "username": username, "password": password });
            let response = http.post(url).json(&body).send().await?;
            if !response.status().is_success() {
                let text = response.text().await?;
                return Err(format!("register failed: {text}").into());
            }
            println!("User registered");
        }
        Commands::ListScripts => {
            let config = load_config()?;
            let url = format!("{}/api/v1/scripts", config.server.trim_end_matches('/'));
            let response = http
                .get(url)
                .header("Authorization", format!("Bearer {}", config.token))
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(format!("list scripts failed: {}", response.text().await?).into());
            }

            let scripts: Vec<ScriptSummary> = response.json().await?;
            for script in scripts {
                println!("{}\t{}\t{}", script.id, script.name, script.language);
            }
        }
        Commands::CreateScript { name, language } => {
            let config = load_config()?;
            let url = format!("{}/api/v1/scripts", config.server.trim_end_matches('/'));
            let response = http
                .post(url)
                .header("Authorization", format!("Bearer {}", config.token))
                .json(&CreateScriptRequest { name, language })
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(format!("create script failed: {}", response.text().await?).into());
            }

            let script: ScriptSummary = response.json().await?;
            println!("Created script {} ({})", script.id, script.name);
        }
        Commands::CreateVersion {
            script_id,
            commit_hash,
            source_bundle_path,
        } => {
            let config = load_config()?;
            let url = format!(
                "{}/api/v1/scripts/{script_id}/versions",
                config.server.trim_end_matches('/')
            );
            let response = http
                .post(url)
                .header("Authorization", format!("Bearer {}", config.token))
                .json(&CreateScriptVersionRequest {
                    commit_hash,
                    source_bundle_path,
                })
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(format!("create version failed: {}", response.text().await?).into());
            }

            let version: race_protocol::ScriptVersionSummary = response.json().await?;
            println!(
                "Created version {} for script {} as v{}",
                version.id, version.script_id, version.version
            );
        }
        Commands::UploadArtifact {
            script_version_id,
            elf_path,
            target,
        } => {
            let config = load_config()?;
            let artifact_id = upload_artifact(
                &http,
                &config,
                script_version_id,
                &target,
                &std::fs::read(&elf_path)?,
            )
            .await?;
            println!("Uploaded artifact {artifact_id}");
        }
        Commands::BuildAndUpload {
            script_version_id,
            bot_dir,
            bin,
            target,
        } => {
            let config = load_config()?;
            let elf = build_elf(&bot_dir, &bin, &target).await?;
            let artifact_id =
                upload_artifact(&http, &config, script_version_id, &target, &elf).await?;
            println!("Built and uploaded artifact {artifact_id}");
        }
    }

    Ok(())
}

async fn upload_artifact(
    http: &reqwest::Client,
    config: &ConnectorConfig,
    script_version_id: i64,
    target: &str,
    elf: &[u8],
) -> Result<i64, Box<dyn std::error::Error>> {
    let url = format!(
        "{}/api/v1/artifacts/upload",
        config.server.trim_end_matches('/')
    );
    let req = UploadArtifactRequest {
        script_version_id,
        target: target.to_string(),
        elf_base64: base64::engine::general_purpose::STANDARD.encode(elf),
        build_meta_json: None,
    };

    let response = http
        .post(url)
        .header("Authorization", format!("Bearer {}", config.token))
        .json(&req)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("upload artifact failed: {}", response.text().await?).into());
    }

    let uploaded: UploadArtifactResponse = response.json().await?;
    Ok(uploaded.artifact_id)
}

async fn build_elf(
    bot_dir: &PathBuf,
    bin: &str,
    target: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let status = tokio::process::Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--bin")
        .arg(bin)
        .arg("--target")
        .arg(target)
        .current_dir(bot_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;

    if !status.success() {
        return Err(format!("cargo build failed with status {status}").into());
    }

    let elf_path = bot_dir
        .join("target")
        .join(target)
        .join("release")
        .join(bin);
    let bytes = std::fs::read(&elf_path)?;
    Ok(bytes)
}

fn load_config() -> Result<ConnectorConfig, Box<dyn std::error::Error>> {
    let path = config_path()?;
    let text = std::fs::read_to_string(path)?;
    let config: ConnectorConfig = toml::from_str(&text)?;
    Ok(config)
}

fn save_config(config: &ConnectorConfig) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(config)?;
    std::fs::write(path, text)?;
    Ok(())
}

fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base = dirs::config_dir().ok_or("failed to locate config dir")?;
    Ok(base.join("programming-game").join("connector.toml"))
}
