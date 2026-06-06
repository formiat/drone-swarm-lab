# Swarm Communication Protocol — M91

Schema version: `swarm_protocol.v1`  
Location: `swarm-comms/src/swarm_protocol.rs`

## Overview

The swarm protocol is a **transport-agnostic**, **typed** message model for
mission-level coordination between drones, a mothership, and a GCS.
It does not prescribe mesh vs LTE vs serial transport; it defines:

- The message vocabulary (`SwarmMessage`)
- Authority semantics (`Lease`)
- Envelope framing and schema versioning (`SwarmMessageEnvelope`)
- Duplicate suppression (`DuplicateSuppressor`)

## Message groups

### Presence and health

| Kind | Direction | Purpose |
|---|---|---|
| `heartbeat` | agent → any | Periodic liveness; includes `mission_state` and `generation` counter |
| `presence` | agent → any | Role and capability advertisement |

### Mission assignment lifecycle

| Kind | Direction | Purpose |
|---|---|---|
| `mission_offer` | GCS → agent | Offer a `MissionCommandPlan` with a TTL for lease authority |
| `mission_accept` | agent → GCS | Accept; returns the `lease_id` granted by the agent |
| `mission_reject` | agent → GCS | Reject with a typed `MissionRejectReason` |
| `mission_result` | agent → GCS | Final outcome (`MavlinkExecutionOutcome`) plus completed segments |

### Ownership and lease management

| Kind | Direction | Purpose |
|---|---|---|
| `ownership_claim` | agent → coord | Announce exclusive ownership of a resource |
| `ownership_release` | agent → coord | Voluntarily surrender ownership |
| `lease_renew` | agent → coord | Extend an expiring lease before it lapses |
| `lease_expired` | coord → agent | Notification that the agent's lease was revoked |

### Segment coordination

| Kind | Direction | Purpose |
|---|---|---|
| `segment_reserve` | agent → coord | Request access to an urban segment (edge) |
| `segment_grant` | coord → agent | Grant; embeds a `Lease` |
| `segment_deny` | coord → agent | Deny with a typed `SegmentDenyReason` and current holder |
| `segment_release` | agent → coord | Release the segment upon completion or abort |

### Progress and status

| Kind | Direction | Purpose |
|---|---|---|
| `progress_update` | agent → GCS | Percentage complete, optional GPS position, tick |
| `replacement_offer` | GCS → agent | Offer a new plan when the current one is superseded |

### Supervisor signals

| Kind | Direction | Purpose |
|---|---|---|
| `abort_notice` | coord → agent | Abort with a typed `AbortAction` |
| `degraded_notice` | agent → coord | Agent reports degraded mode with `DegradedReason` |
| `topology_update` | coord → agent | Push an updated reachable-agents list |

### State reconciliation

| Kind | Direction | Purpose |
|---|---|---|
| `state_request` | any → agent | Request a full state snapshot (used after partition heals) |
| `state_response` | agent → any | Full state: `AgentMissionState`, active leases, completed resources |

## Lease model

A `Lease` grants **exclusive authority** over a named `resource_id`
(a task id, urban edge id, sector id, …) to one holder for a bounded period:

```
now < expires_at  →  Lease::is_valid_at(now) == true
```

### Why leases prevent split-brain

After a network partition, both sides may believe they hold authority over the
same resource. Leases resolve this deterministically: **when `expires_at` is
reached, the holder loses authority regardless of GCS presence**. The GCS does
not need to issue a revocation — time itself acts as the arbiter.

Agents must renew their leases (`lease_renew`) before expiry. If they cannot
reach the coordinator, they should transition to `ContinuingUnderLease` and
eventually to `GcsLost` once the lease lapses.

## Envelope and schema versioning

Every message is wrapped in `SwarmMessageEnvelope`:

```rust
pub struct SwarmMessageEnvelope {
    pub schema_version: String,        // "swarm_protocol.v1"
    pub envelope_id:    String,        // UUID or similar unique id
    pub correlation_id: Option<String>, // links response to request
    pub from:           AgentId,
    pub to:             AgentId,
    pub sent_at:        DateTime<Utc>,
    pub ttl_ticks:      u32,
    pub message:        SwarmMessage,
}
```

- `into_raw_message()` → serialises to JSON, wraps in `RawMessage`
- `from_raw_message()` → deserialises; returns `None` for unknown schema
  versions without panicking

## correlation_id

Links a response back to the originating request:

```
MissionOffer  { envelope_id: "offer-123", correlation_id: None, … }
MissionAccept { envelope_id: "accept-456", correlation_id: Some("offer-123"), … }
```

Receivers that send replies SHOULD set `correlation_id` to the `envelope_id` of
the triggering message.

## Duplicate suppression

`DuplicateSuppressor` maintains a sliding window of the last N `envelope_id`
values. On receiving a message the caller calls `is_duplicate(envelope_id)`:

- Returns `true` → message was already processed; discard silently
- Returns `false` → first delivery; process and record the id

Default window: 256 entries. Oldest entries are evicted to make room for new ones.

## ProtocolRole

`ProtocolRole` mirrors `swarm_command_plane::SwarmCommandRole` (Scout, Observer,
Relay, Leader, Coordinator, Mothership, Carrier, Reserve, Recovery) but is
defined locally in `swarm-comms` to avoid a circular crate dependency.
When bridging to the command plane, convert between the two types at the boundary.

## Replay events (swarm-replay)

Four new variants are added to `swarm_replay::event_log::Event` for observability:

| Variant | Fields | When emitted |
|---|---|---|
| `SwarmProtocolMessage` | `from, to, envelope_id, kind, tick` | Every delivered SwarmMessage |
| `LeaseGranted` | `lease_id, holder, resource_id, expires_at_tick, tick` | Coordinator grants a lease |
| `LeaseExpired` | `lease_id, resource_id, tick` | Lease TTL elapsed |
| `OwnershipConflict` | `resource_id, claimant_a, claimant_b, tick` | Two agents claim the same resource |
