use agentharness_policy::{classify_command, PolicyAction, PolicyDecision};
use agentharness_trace::{
    PolicyDecisionPayload, RunStartedPayload, RunStatus, TraceEventKind, TraceWriter,
};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_TRACE_DEMO_COMMAND: &str = "rm -rf /";
const DEFAULT_RUNS_DIR: &str = "runs";

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
    println!("  agentharness policy check \"<command>\"");
    println!("  agentharness trace demo [--runs-dir <path>] [--command \"<cmd>\"]");
    println!();
    println!("Examples:");
    println!("  agentharness policy check \"cargo test\"");
    println!("  agentharness policy check \"rm -rf /\"");
    println!("  agentharness trace demo");
    println!("  agentharness trace demo --command \"cargo test\"");
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
