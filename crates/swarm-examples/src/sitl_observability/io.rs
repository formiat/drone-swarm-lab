use std::fs;
use std::path::{Path, PathBuf};

use super::events::SitlEventLog;

#[derive(Debug, thiserror::Error)]
pub enum SitlEventLogError {
    #[error("SITL event log directory create failed {path:?}: {message}")]
    CreateDir { path: PathBuf, message: String },
    #[error("SITL event log serialization failed: {message}")]
    Serialize { message: String },
    #[error("SITL event log read failed {path:?}: {message}")]
    Read { path: PathBuf, message: String },
    #[error("SITL event log deserialization failed {path:?}: {message}")]
    Deserialize { path: PathBuf, message: String },
    #[error("SITL event log write failed {path:?}: {message}")]
    Write { path: PathBuf, message: String },
}

pub fn write_sitl_event_log(
    path: impl AsRef<Path>,
    log: &SitlEventLog,
) -> Result<(), SitlEventLogError> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| SitlEventLogError::CreateDir {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    let json = serde_json::to_string_pretty(log).map_err(|error| SitlEventLogError::Serialize {
        message: error.to_string(),
    })?;
    fs::write(path, json).map_err(|error| SitlEventLogError::Write {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
