#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MavlinkMissionEvent {
    HeartbeatSeen,
    MissionClearSent,
    MissionCountSent {
        count: usize,
    },
    MissionItemRequested {
        seq: u16,
    },
    MissionItemSent {
        seq: u16,
    },
    MissionAckReceived {
        result: String,
        accepted: bool,
    },
    CommandSent {
        command: String,
    },
    CommandAckReceived {
        command: String,
        result: String,
        accepted: bool,
    },
    AbortRequested {
        result: String,
    },
}

#[cfg(feature = "mavlink-transport")]
pub trait MavlinkMissionObserver {
    fn on_event(&mut self, event: MavlinkMissionEvent);
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Default)]
pub struct NoopMavlinkMissionObserver;

#[cfg(feature = "mavlink-transport")]
impl MavlinkMissionObserver for NoopMavlinkMissionObserver {
    fn on_event(&mut self, _event: MavlinkMissionEvent) {}
}
