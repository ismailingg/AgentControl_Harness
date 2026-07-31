use agentharness_policy::{classify_command, PolicyAction, PolicyDecision};
use agentharness_trace::{
    ConfirmationResponsePayload, ErrorPayload, PolicyDecisionPayload, RunFinishedPayload,
    RunStartedPayload, RunStatus, TerminalCommandPayload, TraceEventKind, TraceWriter,
};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Instant;

const DEFAULT_TRACE_DEMO_COMMAND: &str = "rm -rf /";
const DEFAULT_RUNS_DIR: &str = "runs";
const OUTPUT_EXCERPT_LIMIT: usize = 4000;

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();

    if args.is_empty() || is_help(&args) {
        print_help();
        return ExitCode::SUCCESS;
    }

    match args.as_slice() {
        [command_group, command, rest @ ..] if command_group == "policy" && command == "check" => {
            check_policy(rest)
        }
        [command_group, command, rest @ ..] if command_group == "trace" && command == "demo" => {
            trace_demo(rest)
        }
        [command, rest @ ..] if command == "run" => run_workflow(rest),
        _ => {
            eprintln!("Unknown command.");
            print_help();
            ExitCode::from(64)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceDemoOptions {
    runs_dir: PathBuf,
    command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunOptions {
    config_path: PathBuf,
    runs_dir: PathBuf,
    yes: bool,
}

#[derive(Debug, Deserialize)]
struct RunConfig {
    workflow: WorkflowConfig,
}

#[derive(Debug, Deserialize)]
struct WorkflowConfig {
    command: String,
}

fn is_help(args: &[String]) -> bool {
    args.len() == 1 && matches!(args[0].as_str(), "help" | "--help" | "-h")
}

fn check_policy(parts: &[String]) -> ExitCode {
    if parts.is_empty() {
        eprintln!("Missing command to check.");
        eprintln!("Usage: agentharness policy check \"rm -rf /\"");
        return ExitCode::from(64);
    }

    let command = parts.join(" ");
    let decision = classify_command(&command);

    println!("Decision: {}", decision.action);
    println!("Risk: {}", decision.risk);
    println!("Rule: {}", decision.matched_rule);
    println!("Reason: {}", decision.reason);

    match decision.action {
        PolicyAction::Block => ExitCode::from(2),
        _ => ExitCode::SUCCESS,
    }
}

fn run_workflow(parts: &[String]) -> ExitCode {
    let options = match parse_run_options(parts) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("Usage: agentharness run <config-file> [--runs-dir <path>] [--yes]");
            return ExitCode::from(64);
        }
    };

    match execute_run(&options) {
        Ok(summary) => {
            println!("Run complete.");
            println!("Run: {}", summary.run_dir.display());
            println!("Status: {}", summary.status);
            println!("Command: {}", summary.command);
            println!("Decision: {}", summary.decision);

            match summary.status {
                "success" => ExitCode::SUCCESS,
                "blocked" => ExitCode::from(2),
                _ => ExitCode::from(1),
            }
        }
        Err(error) => {
            eprintln!("Run failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn trace_demo(parts: &[String]) -> ExitCode {
    let options = match parse_trace_demo_options(parts) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("Usage: agentharness trace demo [--runs-dir <path>] [--command \"<cmd>\"]");
            return ExitCode::from(64);
        }
    };

    match write_trace_demo(&options) {
        Ok(summary) => {
            println!("Trace demo written.");
            println!("Run: {}", summary.run_dir.display());
            println!("Status: {}", summary.status);
            println!("Command: {}", summary.command);
            println!("Decision: {}", summary.decision);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to write trace demo: {error}");
            ExitCode::from(1)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceDemoSummary {
    run_dir: PathBuf,
    status: &'static str,
    command: String,
    decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunSummary {
    run_dir: PathBuf,
    status: &'static str,
    command: String,
    decision: String,
}

fn parse_run_options(parts: &[String]) -> Result<RunOptions, String> {
    let Some(config_path) = parts.first() else {
        return Err("Missing config file.".to_owned());
    };

    if config_path.starts_with("--") {
        return Err("Missing config file before options.".to_owned());
    }

    let mut runs_dir = PathBuf::from(DEFAULT_RUNS_DIR);
    let mut yes = false;
    let mut index = 1;

    while index < parts.len() {
        match parts[index].as_str() {
            "--runs-dir" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("Missing value for --runs-dir.".to_owned());
                };
                runs_dir = PathBuf::from(value);
            }
            "--yes" => yes = true,
            unknown => return Err(format!("Unknown run option: {unknown}")),
        }

        index += 1;
    }

    Ok(RunOptions {
        config_path: PathBuf::from(config_path),
        runs_dir,
        yes,
    })
}

fn parse_trace_demo_options(parts: &[String]) -> Result<TraceDemoOptions, String> {
    let mut runs_dir = PathBuf::from(DEFAULT_RUNS_DIR);
    let mut command = DEFAULT_TRACE_DEMO_COMMAND.to_owned();
    let mut index = 0;

    while index < parts.len() {
        match parts[index].as_str() {
            "--runs-dir" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("Missing value for --runs-dir.".to_owned());
                };
                runs_dir = PathBuf::from(value);
            }
            "--command" => {
                index += 1;
                let Some(value) = parts.get(index) else {
                    return Err("Missing value for --command.".to_owned());
                };
                command = value.to_owned();
            }
            unknown => return Err(format!("Unknown trace demo option: {unknown}")),
        }

        index += 1;
    }

    Ok(TraceDemoOptions { runs_dir, command })
}

fn execute_run(options: &RunOptions) -> Result<RunSummary, Box<dyn std::error::Error>> {
    let config = read_run_config(&options.config_path)?;
    let command = config.workflow.command;
    let mut writer = TraceWriter::create(&options.runs_dir, env!("CARGO_PKG_VERSION"))?;

    writer.append(TraceEventKind::RunStarted(RunStartedPayload {
        agentharness_version: env!("CARGO_PKG_VERSION").to_owned(),
    }))?;

    let decision = classify_command(&command);
    writer.append(TraceEventKind::PolicyDecision(policy_decision_payload(
        &command, &decision,
    )))?;

    let status = match decision.action {
        PolicyAction::Block => RunStatus::Blocked,
        PolicyAction::RequireConfirmation if !options.yes => {
            writer.append(TraceEventKind::ConfirmationResponse(
                ConfirmationResponsePayload {
                    command: command.clone(),
                    approved: false,
                    responder: None,
                },
            ))?;
            RunStatus::Blocked
        }
        PolicyAction::RequireConfirmation => {
            writer.append(TraceEventKind::ConfirmationResponse(
                ConfirmationResponsePayload {
                    command: command.clone(),
                    approved: true,
                    responder: None,
                },
            ))?;
            execute_and_trace_command(&mut writer, &command)?
        }
        PolicyAction::Allow | PolicyAction::Warn => {
            execute_and_trace_command(&mut writer, &command)?
        }
    };

    writer.append(TraceEventKind::RunFinished(RunFinishedPayload { status }))?;
    writer.finish(status)?;

    Ok(RunSummary {
        run_dir: writer.run_dir().to_path_buf(),
        status: run_status_label(status),
        command,
        decision: decision.action.to_string().to_lowercase(),
    })
}

fn read_run_config(path: &PathBuf) -> Result<RunConfig, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&contents)?)
}

fn execute_and_trace_command(
    writer: &mut TraceWriter,
    command: &str,
) -> Result<RunStatus, Box<dyn std::error::Error>> {
    let started_at = Instant::now();

    match shell_command(command).output() {
        Ok(output) => {
            let duration_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            let exit_code = output.status.code();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            writer.append(TraceEventKind::TerminalCommand(TerminalCommandPayload {
                command: command.to_owned(),
                exit_code,
                duration_ms: Some(duration_ms),
                stdout_excerpt: output_excerpt(&stdout),
                stderr_excerpt: output_excerpt(&stderr),
            }))?;

            Ok(if output.status.success() {
                RunStatus::Success
            } else {
                RunStatus::Failed
            })
        }
        Err(error) => {
            writer.append(TraceEventKind::Error(ErrorPayload {
                message: format!("failed to execute command: {error}"),
                code: None,
            }))?;
            Ok(RunStatus::Error)
        }
    }
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("cmd");
    process.args(["/C", command]);
    process
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut process = Command::new("sh");
    process.args(["-c", command]);
    process
}

fn output_excerpt(output: &str) -> Option<String> {
    let output = output.trim();

    if output.is_empty() {
        None
    } else {
        Some(output.chars().take(OUTPUT_EXCERPT_LIMIT).collect())
    }
}

fn write_trace_demo(
    options: &TraceDemoOptions,
) -> Result<TraceDemoSummary, Box<dyn std::error::Error>> {
    let mut writer = TraceWriter::create(&options.runs_dir, env!("CARGO_PKG_VERSION"))?;

    writer.append(TraceEventKind::RunStarted(RunStartedPayload {
        agentharness_version: env!("CARGO_PKG_VERSION").to_owned(),
    }))?;

    let decision = classify_command(&options.command);
    writer.append(TraceEventKind::PolicyDecision(policy_decision_payload(
        &options.command,
        &decision,
    )))?;

    let status = trace_demo_status_for_policy_action(decision.action);
    writer.finish(status)?;

    Ok(TraceDemoSummary {
        run_dir: writer.run_dir().to_path_buf(),
        status: run_status_label(status),
        command: options.command.clone(),
        decision: decision.action.to_string().to_lowercase(),
    })
}

fn policy_decision_payload(command: &str, decision: &PolicyDecision) -> PolicyDecisionPayload {
    PolicyDecisionPayload {
        command: command.to_owned(),
        action: decision.action.to_string().to_lowercase(),
        risk: decision.risk.to_string().to_lowercase(),
        rule: decision.matched_rule.to_owned(),
        reason: decision.reason.to_owned(),
    }
}

fn trace_demo_status_for_policy_action(action: PolicyAction) -> RunStatus {
    // In trace demo, Success means "classified as non-blocking", not
    // "executed successfully". Revisit this once agentharness run executes commands.
    match action {
        PolicyAction::Allow | PolicyAction::Warn => RunStatus::Success,
        PolicyAction::RequireConfirmation | PolicyAction::Block => RunStatus::Blocked,
    }
}

fn run_status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "running",
        RunStatus::Success => "success",
        RunStatus::Failed => "failed",
        RunStatus::Blocked => "blocked",
        RunStatus::Error => "error",
    }
}

fn print_help() {
    println!("AgentHarness");
    println!();
    println!("Usage:");
    println!("  agentharness run <config-file> [--runs-dir <path>] [--yes]");
    println!("  agentharness policy check \"<command>\"");
    println!("  agentharness trace demo [--runs-dir <path>] [--command \"<cmd>\"]");
    println!();
    println!("Examples:");
    println!("  agentharness run examples/risky-command.yaml");
    println!("  agentharness policy check \"cargo test\"");
    println!("  agentharness policy check \"rm -rf /\"");
    println!("  agentharness trace demo");
    println!("  agentharness trace demo --command \"cargo test\"");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_run_options_with_defaults() {
        let options =
            parse_run_options(&["examples/risky-command.yaml".to_owned()]).expect("run parses");

        assert_eq!(
            options.config_path,
            PathBuf::from("examples/risky-command.yaml")
        );
        assert_eq!(options.runs_dir, PathBuf::from(DEFAULT_RUNS_DIR));
        assert!(!options.yes);
    }

    #[test]
    fn parses_run_options_with_overrides() {
        let options = parse_run_options(&[
            "examples/risky-command.yaml".to_owned(),
            "--runs-dir".to_owned(),
            "tmp-runs".to_owned(),
            "--yes".to_owned(),
        ])
        .expect("run parses");

        assert_eq!(options.runs_dir, PathBuf::from("tmp-runs"));
        assert!(options.yes);
    }

    #[test]
    fn parses_trace_demo_defaults() {
        let options = parse_trace_demo_options(&[]).expect("defaults should parse");

        assert_eq!(options.runs_dir, PathBuf::from(DEFAULT_RUNS_DIR));
        assert_eq!(options.command, DEFAULT_TRACE_DEMO_COMMAND);
    }

    #[test]
    fn parses_trace_demo_overrides() {
        let options = parse_trace_demo_options(&[
            "--runs-dir".to_owned(),
            "tmp-runs".to_owned(),
            "--command".to_owned(),
            "cargo test".to_owned(),
        ])
        .expect("overrides should parse");

        assert_eq!(options.runs_dir, PathBuf::from("tmp-runs"));
        assert_eq!(options.command, "cargo test");
    }

    #[test]
    fn maps_trace_demo_status_from_policy_action() {
        assert_eq!(
            trace_demo_status_for_policy_action(PolicyAction::Allow),
            RunStatus::Success
        );
        assert_eq!(
            trace_demo_status_for_policy_action(PolicyAction::Warn),
            RunStatus::Success
        );
        assert_eq!(
            trace_demo_status_for_policy_action(PolicyAction::RequireConfirmation),
            RunStatus::Blocked
        );
        assert_eq!(
            trace_demo_status_for_policy_action(PolicyAction::Block),
            RunStatus::Blocked
        );
    }

    #[test]
    fn converts_policy_decision_to_lowercase_trace_payload() {
        let decision = classify_command("rm -rf /");
        let payload = policy_decision_payload("rm -rf /", &decision);

        assert_eq!(payload.command, "rm -rf /");
        assert_eq!(payload.action, "block");
        assert_eq!(payload.risk, "critical");
        assert_eq!(payload.rule, "destructive-root-delete");
    }

    #[test]
    fn output_excerpt_omits_empty_output_and_truncates_long_output() {
        assert_eq!(output_excerpt(" \n\t"), None);

        let long_output = "x".repeat(OUTPUT_EXCERPT_LIMIT + 10);
        assert_eq!(
            output_excerpt(&long_output)
                .expect("excerpt should exist")
                .len(),
            OUTPUT_EXCERPT_LIMIT
        );
    }
}
