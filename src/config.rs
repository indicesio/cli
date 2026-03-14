use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_API_BASE: &str = "https://api.indices.io";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    #[default]
    Markdown,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub timeout_seconds: Option<u64>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            api_base: Some(DEFAULT_API_BASE.to_string()),
            api_key: None,
            timeout_seconds: Some(DEFAULT_TIMEOUT_SECONDS),
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to determine configuration directory")]
    ConfigDirUnavailable,
    #[error("failed to read config file {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
    #[error("failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("failed to write config file {path}: {source}")]
    Write { path: String, source: io::Error },
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeOverrides<'a> {
    pub api_base: Option<&'a str>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug)]
pub struct ConfigStore {
    path: PathBuf,
    data: ConfigFile,
}

impl ConfigStore {
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_path()?;
        if !path.exists() {
            let mut store = Self {
                path,
                data: ConfigFile::default(),
            };
            store.persist()?;
            return Ok(store);
        }

        let raw = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;

        let data = toml::from_str::<ConfigFile>(&raw).map_err(|source| ConfigError::Parse {
            path: path.display().to_string(),
            source,
        })?;

        let mut store = Self { path, data };
        store.persist()?;
        Ok(store)
    }

    pub fn resolve_runtime(
        &self,
        overrides: &RuntimeOverrides<'_>,
    ) -> Result<RuntimeConfig, ConfigError> {
        let api_base = overrides
            .api_base
            .map(ToOwned::to_owned)
            .or_else(|| self.data.api_base.clone())
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());

        let timeout_seconds = overrides
            .timeout_seconds
            .or(self.data.timeout_seconds)
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS);

        Ok(RuntimeConfig {
            api_base,
            api_key: self.data.api_key.clone(),
            timeout_seconds,
        })
    }

    pub fn set_api_key(
        &mut self,
        api_key: String,
        api_base: Option<&str>,
        timeout_seconds: Option<u64>,
    ) -> Result<(), ConfigError> {
        self.data.api_key = Some(api_key);

        if let Some(api_base) = api_base {
            self.data.api_base = Some(api_base.to_string());
        }

        if let Some(timeout_seconds) = timeout_seconds {
            self.data.timeout_seconds = Some(timeout_seconds);
        }

        self.persist()
    }

    pub fn clear_api_key(&mut self) -> Result<bool, ConfigError> {
        let existed = self.data.api_key.take().is_some();
        self.persist()?;
        Ok(existed)
    }

    fn persist(&mut self) -> Result<(), ConfigError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
                path: parent.display().to_string(),
                source,
            })?;
        }

        let rendered = toml::to_string_pretty(&self.data)?;
        write_config(&self.path, rendered.as_bytes())
    }
}

fn config_path() -> Result<PathBuf, ConfigError> {
    if let Ok(path) = std::env::var("INDICES_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }

    Ok(base_config_dir()?.join(CONFIG_FILE_NAME))
}

fn base_config_dir() -> Result<PathBuf, ConfigError> {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".config").join("indices"));
        }
    }

    let dirs = BaseDirs::new().ok_or(ConfigError::ConfigDirUnavailable)?;
    Ok(dirs.config_dir().join("indices"))
}

fn write_config(path: &PathBuf, bytes: &[u8]) -> Result<(), ConfigError> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|source| ConfigError::Write {
                path: path.display().to_string(),
                source,
            })?;

        file.write_all(bytes).map_err(|source| ConfigError::Write {
            path: path.display().to_string(),
            source,
        })?;

        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions).map_err(|source| ConfigError::Write {
            path: path.display().to_string(),
            source,
        })?;

        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(path, bytes).map_err(|source| ConfigError::Write {
            path: path.display().to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_has_single_auth_shape() {
        let config = ConfigFile::default();
        assert_eq!(config.api_base.as_deref(), Some(DEFAULT_API_BASE));
    }

    #[test]
    fn runtime_defaults_do_not_depend_on_output_config() {
        let config = ConfigFile::default();
        let store = ConfigStore {
            path: PathBuf::from("/tmp/indices/config.toml"),
            data: config,
        };

        let runtime = store
            .resolve_runtime(&RuntimeOverrides {
                api_base: None,
                timeout_seconds: None,
            })
            .expect("runtime should resolve");

        assert_eq!(runtime.api_base, DEFAULT_API_BASE);
        assert_eq!(runtime.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }
}
