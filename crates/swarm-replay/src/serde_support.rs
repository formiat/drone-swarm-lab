use crate::event_log::EventLog;
use std::io;

/// Serialize an EventLog to a JSON string.
pub fn to_json(log: &EventLog) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(log)
}

/// Deserialize an EventLog from a JSON string.
pub fn from_json(s: &str) -> Result<EventLog, serde_json::Error> {
    serde_json::from_str(s)
}

/// Write an EventLog to a file.
pub fn write_to_file(log: &EventLog, path: &std::path::Path) -> io::Result<()> {
    let json = to_json(log).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)
}

/// Read an EventLog from a file.
pub fn read_from_file(path: &std::path::Path) -> io::Result<EventLog> {
    let json = std::fs::read_to_string(path)?;
    from_json(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_log::{Event, EventLogBuilder};
    use swarm_types::AgentId;

    #[test]
    fn round_trip_json() {
        let mut builder = EventLogBuilder::new("rt", 0, "s");
        builder.push(Event::TickStart { tick: 0 });
        let log = builder.build();

        let json = to_json(&log).unwrap();
        let restored = from_json(&json).unwrap();
        assert_eq!(log, restored);
    }

    #[test]
    fn write_and_read_file() {
        let mut builder = EventLogBuilder::new("file-test", 99, "coverage");
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a".to_owned()),
            tick: 5,
        });
        let log = builder.build();

        let path =
            std::env::temp_dir().join(format!("swarm_replay_test_{}.json", std::process::id()));
        write_to_file(&log, &path).unwrap();
        let restored = read_from_file(&path).unwrap();
        assert_eq!(log, restored);
        let _ = std::fs::remove_file(&path);
    }
}
