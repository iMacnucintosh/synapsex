use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// Human names for onboard slots, keyed by `"serial#slotN"` or just `"serial"` for slot 1.
    #[serde(default)]
    pub slot_names: HashMap<String, String>,
    #[serde(default)]
    pub keep_running_in_background: bool,
}

impl AppConfig {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synapsex")
            .join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn slot_key(serial: &str, slot: u8) -> String {
        if slot <= 1 {
            serial.to_string()
        } else {
            format!("{serial}#slot{slot}")
        }
    }

    pub fn name_for(&self, serial: &str, slot: u8) -> String {
        let key = Self::slot_key(serial, slot);
        self.slot_names
            .get(&key)
            .cloned()
            .unwrap_or_else(|| synapsex_i18n::default_profile_name(slot))
    }

    pub fn set_name(&mut self, serial: &str, slot: u8, name: String) {
        self.slot_names
            .insert(Self::slot_key(serial, slot), name);
    }
}
