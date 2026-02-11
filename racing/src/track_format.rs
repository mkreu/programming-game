use bevy::math::Vec2;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackFile {
    #[serde(default)]
    pub metadata: TrackMetadata,
    pub control_points: Vec<[f32; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackMetadata {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default = "default_track_width")]
    pub track_width: f32,
    #[serde(default = "default_kerb_width")]
    pub kerb_width: f32,
}

impl Default for TrackMetadata {
    fn default() -> Self {
        Self {
            name: default_name(),
            author: String::new(),
            track_width: default_track_width(),
            kerb_width: default_kerb_width(),
        }
    }
}

fn default_name() -> String {
    "Untitled".to_string()
}

fn default_track_width() -> f32 {
    12.0
}

fn default_kerb_width() -> f32 {
    0.5
}

impl TrackFile {
    /// Create a new empty track with default metadata.
    pub fn new_empty(name: &str) -> Self {
        Self {
            metadata: TrackMetadata {
                name: name.to_string(),
                author: String::new(),
                track_width: default_track_width(),
                kerb_width: default_kerb_width(),
            },
            control_points: Vec::new(),
        }
    }

    /// Load a track from a TOML file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        toml::from_str(&text)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
    }

    /// Save this track to a TOML file.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize track: {}", e))?;
        std::fs::write(path, text)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
    }

    /// Get control points as Vec2 (world coordinates, no transform needed).
    pub fn control_points_vec2(&self) -> Vec<Vec2> {
        self.control_points
            .iter()
            .map(|&[x, y]| Vec2::new(x, y))
            .collect()
    }
}
