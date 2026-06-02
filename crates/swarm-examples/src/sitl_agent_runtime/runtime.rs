use std::path::Path;

use super::cli::{parse_args, AgentRuntimeOptions, LifecycleMode};
use super::connection_and_reports::run_connection;
use super::mock::{apply_start_delay, run_mock};
use crate::sitl_multi_agent::{
    agent_config, build_multi_agent_manifest, load_multi_agent_config, MultiAgentLifecycle,
    MultiAgentSitlAgentConfig,
};
use crate::sitl_plan::{
    build_sitl_plan_for_task_ids, classify_connection_string, first_sitl_entry,
    format_dry_run_plan, load_sitl_suite, SitlConnectionClass, SitlError, SitlMode,
};
use crate::sitl_safety::{
    load_sitl_safety_config, validate_pre_upload_safety, validate_pre_upload_safety_for_task_ids,
};

pub fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let suite = load_sitl_suite(&cli.scenario)?;
    let multi_agent_config = cli
        .multi_agent_config
        .as_deref()
        .map(load_multi_agent_config)
        .transpose()?;

    let mut lifecycle = cli.lifecycle;
    let mut runtime_options = AgentRuntimeOptions::default();
    let mut mode = cli.mode.clone();
    let mut safety_task_ids: Option<Vec<String>> = None;
    let plan = if let Some(config) = multi_agent_config.as_ref() {
        let config_path = cli.multi_agent_config.as_ref().expect("config path exists");
        let manifest = build_multi_agent_manifest(&suite, &cli.scenario, config_path, config)?;
        let agent = agent_config(config, &cli.agent_id)?;
        if !cli.lifecycle_from_cli {
            lifecycle.mode = lifecycle_mode_from_config(agent.lifecycle);
        }
        runtime_options = runtime_options_from_config(agent);
        if mode.is_none() {
            mode = Some(SitlMode::Connection {
                addr: agent.connection_string.clone(),
            });
        }
        safety_task_ids = Some(agent.task_ids.clone());
        if matches!(mode, Some(SitlMode::DryRun)) {
            let agent_manifest = manifest
                .agents
                .iter()
                .find(|item| item.agent_id == cli.agent_id)
                .expect("validated manifest contains agent");
            eprintln!(
                "Multi-agent SITL: agent={} system_id={} component_id={} connection={} lifecycle={:?} start_delay_ms={} task_ids={}",
                agent_manifest.agent_id,
                agent_manifest.system_id,
                agent_manifest.component_id,
                agent_manifest.connection_string,
                agent_manifest.lifecycle,
                agent_manifest.start_delay_ms,
                agent_manifest.task_ids.join(",")
            );
        }
        build_sitl_plan_for_task_ids(&suite, &cli.scenario, &cli.agent_id, &agent.task_ids)?
    } else {
        crate::sitl_plan::build_sitl_plan(&suite, &cli.scenario, cli.agent_id.clone())?
    };

    let mode = mode.ok_or(SitlError::MissingMode)?;
    if cli.run_report.is_some() && lifecycle.mode != LifecycleMode::Execute {
        return Err(SitlError::RunReportRequiresExecute {
            option: "--run-report",
        });
    }

    if let SitlMode::Connection { addr } = &mode {
        enforce_hardware_candidate_boundary(addr, cli.allow_hardware_candidate)?;
        let safety_config = load_sitl_safety_config(cli.safety_config.as_deref().map(Path::new))?;
        let entry = first_sitl_entry(&suite, &cli.scenario)?;
        if let Some(task_ids) = safety_task_ids.as_ref() {
            validate_pre_upload_safety_for_task_ids(
                entry,
                &plan.agent_id,
                &safety_config,
                task_ids,
            )?;
        } else {
            validate_pre_upload_safety(entry, &plan.agent_id, &safety_config)?;
        }
    }

    match mode {
        SitlMode::Mock => {
            apply_start_delay(runtime_options.start_delay_ms);
            run_mock(&plan, cli.replay_log.as_deref())
        }
        SitlMode::DryRun => {
            print!("{}", format_dry_run_plan(&plan));
            Ok(())
        }
        SitlMode::Connection { addr } => run_connection(
            &plan,
            &addr,
            &lifecycle,
            runtime_options,
            cli.run_report.as_deref(),
            cli.replay_log.as_deref(),
        ),
    }
}

fn enforce_hardware_candidate_boundary(
    addr: &str,
    allow_hardware_candidate: bool,
) -> Result<(), SitlError> {
    let class = classify_connection_string(addr)?;
    if matches!(class, SitlConnectionClass::HardwareCandidate) {
        if allow_hardware_candidate {
            print_hardware_candidate_warning(addr, class);
        } else {
            return Err(SitlError::HardwareCandidateRequiresExplicitAllow {
                addr: addr.to_owned(),
                class: class.name(),
            });
        }
    }
    Ok(())
}

fn print_hardware_candidate_warning(addr: &str, class: SitlConnectionClass) {
    eprintln!(
        "WARNING: connection '{addr}' is classified as {}. This may target real hardware or a remote endpoint. This project is not hardware-ready, does not provide a certified safety layer, and requires the operator checklist in docs/HARDWARE_READINESS.md before any hardware experiment.",
        class.name()
    );
}

fn lifecycle_mode_from_config(lifecycle: MultiAgentLifecycle) -> LifecycleMode {
    match lifecycle {
        MultiAgentLifecycle::UploadOnly => LifecycleMode::UploadOnly,
        MultiAgentLifecycle::Execute => LifecycleMode::Execute,
    }
}

fn runtime_options_from_config(config: &MultiAgentSitlAgentConfig) -> AgentRuntimeOptions {
    AgentRuntimeOptions {
        start_delay_ms: config.start_delay_ms,
        target_system: config.system_id,
        target_component: config.component_id,
    }
}
