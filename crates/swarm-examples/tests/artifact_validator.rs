use std::fs;
use std::path::{Path, PathBuf};

use swarm_examples::artifact_validator::{
    validate_artifact_pack, ArtifactPackPaths, ArtifactValidationMode, ArtifactValidationOptions,
    RULE_BUILD_PROFILE_MISSING, RULE_COMPLETED_TASK_MISSING_EVENT, RULE_FINAL_STATUS_MISMATCH,
    RULE_MANIFEST_MISSING, RULE_REPLACEMENT_SEQ_MISMATCH, RULE_REPLAY_SUMMARY_COUNT_MISMATCH,
    RULE_SAFETY_REPORT_MISSING,
};
use swarm_examples::sitl_multi_agent::{
    MultiAgentLifecycle, MultiAgentSitlManifest, MultiAgentSitlManifestAgent, SitlArtifactMetadata,
    TaskOwnershipSummary, MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION,
};
use swarm_examples::sitl_observability::{
    format_sitl_summary, summarize_sitl_event_log, SitlEvent, SitlEventLog, SitlEventLogMode,
};
use swarm_examples::sitl_report::{
    SitlMultiAgentAgentReport, SitlMultiAgentReallocationReport, SitlMultiAgentRunReport,
};
use swarm_safety::preflight::SafetyValidationReport;

#[test]
fn valid_tiny_supervisor_pack_passes() {
    let fixture = ArtifactFixture::new();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert!(report.passed, "{:?}", report.violations);
}

#[test]
fn missing_manifest_fails_with_rule_id() {
    let fixture = ArtifactFixture::new();
    fs::remove_file(fixture.output_dir.join("manifest.json")).unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_MANIFEST_MISSING);
}

#[test]
fn missing_manifest_metadata_fails_in_strict_mode() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest.artifact_metadata.build_profile.clear();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_BUILD_PROFILE_MISSING);
}

#[test]
fn final_status_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_report(|report| {
        report.final_status = "failed".to_owned();
        report.overall_status = "failed".to_owned();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_FINAL_STATUS_MISMATCH);
}

#[test]
fn completed_task_count_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_report(|report| {
        report.total_completed_tasks = 1;
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_COMPLETED_TASK_MISSING_EVENT);
}

#[test]
fn replay_summary_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fs::write(fixture.output_dir.join("replay-summary.txt"), "wrong\n").unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_REPLAY_SUMMARY_COUNT_MISMATCH);
}

#[test]
fn replacement_seq_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_event_log(|log| {
        log.events.push(SitlEvent::MultiAgentMissionItemSent {
            step: 5,
            agent_id: "agent-1".to_owned(),
            seq: 2,
            task_id: Some("wp-0".to_owned()),
        });
        log.events.push(SitlEvent::MultiAgentTaskCompleted {
            step: 6,
            agent_id: "agent-1".to_owned(),
            seq: 0,
            task_id: "wp-0".to_owned(),
        });
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_REPLACEMENT_SEQ_MISMATCH);
}

#[test]
fn missing_safety_report_fails_for_supervisor_run() {
    let fixture = ArtifactFixture::new();
    fs::remove_file(fixture.output_dir.join("safety_validation_report.v1.json")).unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::SupervisorRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SAFETY_REPORT_MISSING);
}

fn assert_rule(
    report: &swarm_examples::artifact_validator::ArtifactValidationReport,
    rule_id: &str,
) {
    assert!(
        report
            .violations
            .iter()
            .any(|violation| violation.rule_id == rule_id),
        "missing {rule_id}; violations={:?}",
        report.violations
    );
}

struct ArtifactFixture {
    _tempdir: tempfile::TempDir,
    output_dir: PathBuf,
}

impl ArtifactFixture {
    fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let output_dir = tempdir.path().join("run-1");
        fs::create_dir_all(&output_dir).unwrap();
        let fixture = Self {
            _tempdir: tempdir,
            output_dir,
        };
        fixture.write_manifest(|_| {});
        fixture.write_event_log(|_| {});
        fixture.write_report(|_| {});
        fixture.write_replay_summary();
        fixture.write_safety_report();
        fs::write(fixture.output_dir.join("scenario.snapshot.json"), "{}\n").unwrap();
        fs::write(fixture.output_dir.join("config.snapshot.json"), "{}\n").unwrap();
        fs::write(
            fixture.output_dir.join("command.txt"),
            "sitl_supervisor --mock\n",
        )
        .unwrap();
        fixture
    }

    fn write_manifest(&self, mutate: impl FnOnce(&mut MultiAgentSitlManifest)) {
        let mut manifest = MultiAgentSitlManifest {
            schema_version: MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION.to_owned(),
            scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
            scenario_name: "sitl_multi_agent".to_owned(),
            mission: "sitl".to_owned(),
            profile: "multi-agent".to_owned(),
            agents_count: 2,
            agents: vec![
                manifest_agent("agent-0", 1, &["wp-0"]),
                manifest_agent("agent-1", 2, &["wp-1"]),
            ],
            ownership_summary: TaskOwnershipSummary {
                total_pose_tasks: 2,
                assigned_task_count: 2,
                unassigned_pose_tasks: Vec::new(),
                duplicate_task_ids: Vec::new(),
            },
            artifact_metadata: SitlArtifactMetadata {
                command: vec!["sitl_supervisor".to_owned(), "--mock".to_owned()],
                git_commit: Some("0123456789abcdef".to_owned()),
                build_profile: "debug".to_owned(),
                run_id: Some("run-1".to_owned()),
                scenario_snapshot_path: Some(PathBuf::from("scenario.snapshot.json")),
                config_snapshot_path: Some(PathBuf::from("config.snapshot.json")),
                command_path: Some(PathBuf::from("command.txt")),
            },
        };
        mutate(&mut manifest);
        write_json(&self.output_dir.join("manifest.json"), &manifest);
    }

    fn write_event_log(&self, mutate: impl FnOnce(&mut SitlEventLog)) {
        let mut log = base_event_log();
        mutate(&mut log);
        write_json(&self.output_dir.join("events.sitl-log.json"), &log);
        let summary = summarize_sitl_event_log(&log);
        let mut report = base_report(summary);
        report.total_completed_tasks = log
            .events
            .iter()
            .filter(|event| matches!(event, SitlEvent::MultiAgentTaskCompleted { .. }))
            .count();
        write_json(&self.output_dir.join("run-report.json"), &report);
        fs::write(
            self.output_dir.join("replay-summary.txt"),
            format!("{}\n", format_sitl_summary(&summarize_sitl_event_log(&log))),
        )
        .unwrap();
    }

    fn write_report(&self, mutate: impl FnOnce(&mut SitlMultiAgentRunReport)) {
        let log = read_fixture_log(&self.output_dir);
        let mut report = base_report(summarize_sitl_event_log(&log));
        mutate(&mut report);
        write_json(&self.output_dir.join("run-report.json"), &report);
    }

    fn write_replay_summary(&self) {
        let log = read_fixture_log(&self.output_dir);
        fs::write(
            self.output_dir.join("replay-summary.txt"),
            format!("{}\n", format_sitl_summary(&summarize_sitl_event_log(&log))),
        )
        .unwrap();
    }

    fn write_safety_report(&self) {
        write_json(
            &self.output_dir.join("safety_validation_report.v1.json"),
            &SafetyValidationReport::ok(),
        );
    }
}

fn manifest_agent(agent_id: &str, system_id: u8, task_ids: &[&str]) -> MultiAgentSitlManifestAgent {
    MultiAgentSitlManifestAgent {
        agent_id: agent_id.to_owned(),
        system_id,
        component_id: 1,
        connection_string: format!("udpin:127.0.0.1:1455{system_id}"),
        start_delay_ms: 0,
        lifecycle: MultiAgentLifecycle::Execute,
        task_ids: task_ids
            .iter()
            .map(|task_id| (*task_id).to_owned())
            .collect(),
        waypoint_count: task_ids.len(),
        waypoints: Vec::new(),
        standalone_command: Vec::new(),
    }
}

fn base_event_log() -> SitlEventLog {
    SitlEventLog {
        schema_version: "sitl_event_log.v1".to_owned(),
        run_id: "run-1".to_owned(),
        scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
        scenario_name: "sitl_multi_agent".to_owned(),
        mission: "sitl".to_owned(),
        profile: "multi-agent".to_owned(),
        agent_id: "supervisor".to_owned(),
        connection_string: None,
        mode: SitlEventLogMode::ConnectionExecute,
        events: vec![
            SitlEvent::MultiAgentRunStarted {
                step: 0,
                agent_count: 2,
                scenario: "sitl_multi_agent".to_owned(),
            },
            SitlEvent::MultiAgentMissionItemSent {
                step: 1,
                agent_id: "agent-0".to_owned(),
                seq: 0,
                task_id: Some("wp-0".to_owned()),
            },
            SitlEvent::MultiAgentTaskCompleted {
                step: 2,
                agent_id: "agent-0".to_owned(),
                seq: 0,
                task_id: "wp-0".to_owned(),
            },
            SitlEvent::MultiAgentMissionItemSent {
                step: 3,
                agent_id: "agent-1".to_owned(),
                seq: 0,
                task_id: Some("wp-1".to_owned()),
            },
            SitlEvent::MultiAgentTaskCompleted {
                step: 4,
                agent_id: "agent-1".to_owned(),
                seq: 0,
                task_id: "wp-1".to_owned(),
            },
            SitlEvent::MultiAgentRunFinished {
                step: 5,
                overall_status: "completed".to_owned(),
            },
        ],
    }
}

fn base_report(
    summary: swarm_examples::sitl_observability::SitlEventLogSummary,
) -> SitlMultiAgentRunReport {
    SitlMultiAgentRunReport {
        schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
        run_id: "run-1".to_owned(),
        scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
        scenario_name: "sitl_multi_agent".to_owned(),
        config_path: PathBuf::from("scenarios/sitl.multi-agent.config.json"),
        mission: "sitl".to_owned(),
        profile: "multi-agent".to_owned(),
        mode: "connection_execute".to_owned(),
        agents: vec![agent_report("agent-0", 1), agent_report("agent-1", 2)],
        total_completed_tasks: 2,
        failed_agents: 0,
        aborted_agents: 0,
        overall_status: "completed".to_owned(),
        event_log_path: Some(PathBuf::from("events.sitl-log.json")),
        task_ownership: TaskOwnershipSummary {
            total_pose_tasks: 2,
            assigned_task_count: 2,
            unassigned_pose_tasks: Vec::new(),
            duplicate_task_ids: Vec::new(),
        },
        events_summary: summary,
        final_status: "completed".to_owned(),
        reallocation: SitlMultiAgentReallocationReport::default(),
        limitations: vec!["local PX4/SIH only".to_owned()],
        known_limitations: vec!["local PX4/SIH only".to_owned()],
    }
}

fn agent_report(agent_id: &str, system_id: u8) -> SitlMultiAgentAgentReport {
    SitlMultiAgentAgentReport {
        agent_id: agent_id.to_owned(),
        connection_string: format!("udpin:127.0.0.1:1455{system_id}"),
        system_id,
        component_id: 1,
        lifecycle: "execute".to_owned(),
        mission_item_count: 1,
        completed_task_count: 1,
        final_status: "completed".to_owned(),
        error: None,
    }
}

fn read_fixture_log(output_dir: &Path) -> SitlEventLog {
    let text = fs::read_to_string(output_dir.join("events.sitl-log.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    let json = serde_json::to_string_pretty(value).unwrap();
    fs::write(path, json).unwrap();
}
