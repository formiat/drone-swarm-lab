use std::fs;
use std::path::Path;

use super::io::SitlEventLogError;
use super::SitlEventLog;

pub fn read_sitl_event_log(path: impl AsRef<Path>) -> Result<SitlEventLog, SitlEventLogError> {
    let path = path.as_ref();
    let json = fs::read_to_string(path).map_err(|error| SitlEventLogError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&json).map_err(|error| SitlEventLogError::Deserialize {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}
