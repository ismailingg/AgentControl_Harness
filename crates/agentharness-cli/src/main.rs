use agentharness_policy::{classify_command, PolicyAction, PolicyDecision};
use agentharness_trace::{
    read_events, ConfirmationResponsePayload, ErrorPayload, PolicyDecisionPayload,
    RunFinishedPayload, RunMetadata, RunStartedPayload, RunStatus, TerminalCommandPayload,
    TraceEventKind, TraceWriter,
};
use serde::Deserialize;
use std::env;
use std::fs;
use std::io::{self, Write};
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
        [command, rest @ ..] if command == "report" => report_run(rest),
        [command, rest @ ..] if command == "ci" => ci_run(rest),
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportOptions {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RunConfig {
    workflow: WorkflowConfig,
}

#[derive(Debug, Deserialize)]
struct WorkflowConfig {
    steps: Vec<String>,
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
            println!();
            println!("Steps:");
            for (index, step) in summary.steps.iter().enumerate() {
                println!(
                    "  {}. {} -> {} ({})",
                    index + 1,
                    step.command,
                    step.status,
                    step.decision
                );
            }

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

fn report_run(parts: &[String]) -> ExitCode {
    let options = match parse_report_options(parts) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("Usage: agentharness report <runs-dir-or-run-dir>");
            return ExitCode::from(64);
        }
    };

    match build_report(&options) {
        Ok(report) => {
            print_report(&report);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Failed to read report: {error}");
            ExitCode::from(1)
        }
    }
}

fn ci_run(parts: &[String]) -> ExitCode {
    let options = match parse_report_options(parts) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("Usage: agentharness ci <runs-dir-or-run-dir>");
            return ExitCode::from(64);
        }
    };

    match build_report(&options) {
        Ok(report) => {
            print_ci_report(&report);

            if evaluations_passed(&report.evaluations) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("Failed to read report: {error}");
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
    steps: Vec<StepSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StepSummary {
    command: String,
    decision: String,
    status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunReport {
    run_dir: PathBuf,
    metadata: RunMetadata,
    event_count: usize,
    policy_decision_count: usize,
    terminal_command_count: usize,
    confirmation_response_count: usize,
    blocked_command_count: usize,
    warning_count: usize,
    failed_terminal_command_count: usize,
    policy_decisions: Vec<PolicyDecisionPayload>,
    terminal_commands: Vec<TerminalCommandPayload>,
    confirmation_responses: Vec<ConfirmationResponsePayload>,
    evaluations: Vec<ReportEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportEvaluation {
    name: &'static str,
    passed: bool,
    detail: String,
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

fn parse_report_options(parts: &[String]) -> Result<ReportOptions, String> {
    let Some(path) = parts.first() else {
        return Err("Missing report path.".to_owned());
    };

    if parts.len() > 1 {
        return Err("Too many arguments for report.".to_owned());
    }

    Ok(ReportOptions {
        path: PathBuf::from(path),
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

fn build_report(options: &ReportOptions) -> Result<RunReport, Box<dyn std::error::Error>> {
    let run_dir = resolve_report_run_dir(&options.path)?;
    let metadata = read_run_metadata(&run_dir)?;
    let events = read_events(run_dir.join("events.jsonl"))?;
    let mut policy_decisions = Vec::new();
    let mut terminal_commands = Vec::new();
    let mut confirmation_responses = Vec::new();

    for event in &events {
        match &event.kind {
            TraceEventKind::PolicyDecision(payload) => policy_decisions.push(payload.clone()),
            TraceEventKind::TerminalCommand(payload) => terminal_commands.push(payload.clone()),
            TraceEventKind::ConfirmationResponse(payload) => {
                confirmation_responses.push(payload.clone());
            }
            _ => {}
        }
    }

    let blocked_command_count = policy_decisions
        .iter()
        .filter(|decision| decision.action == "block")
        .count();
    let warning_count = policy_decisions
        .iter()
        .filter(|decision| decision.action == "warn")
        .count();
    let failed_terminal_command_count = terminal_commands
        .iter()
        .filter(|command| command.exit_code != Some(0))
        .count();

    let evaluations = report_evaluations(
        metadata.status,
        blocked_command_count,
        failed_terminal_command_count,
    );

    Ok(RunReport {
        run_dir,
        metadata,
        event_count: events.len(),
        policy_decision_count: policy_decisions.len(),
        terminal_command_count: terminal_commands.len(),
        confirmation_response_count: confirmation_responses.len(),
        blocked_command_count,
        warning_count,
        failed_terminal_command_count,
        policy_decisions,
        terminal_commands,
        confirmation_responses,
        evaluations,
    })
}

fn resolve_report_run_dir(path: &PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let latest_path = path.join("latest.txt");

    if latest_path.exists() {
        let latest_run = fs::read_to_string(latest_path)?;
        Ok(path.join(latest_run.trim()))
    } else {
        Ok(path.clone())
    }
}

fn read_run_metadata(run_dir: &PathBuf) -> Result<RunMetadata, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(run_dir.join("metadata.json"))?;
    Ok(serde_json::from_str(&contents)?)
}

fn print_report(report: &RunReport) {
    println!("Run: {}", report.metadata.run_id);
    println!("Path: {}", report.run_dir.display());
    println!("Status: {}", run_status_label(report.metadata.status));
    println!("Started: {}", report.metadata.started_at);

    if let Some(finished_at) = &report.metadata.finished_at {
        println!("Finished: {finished_at}");
    }

    println!();
    println!("Events: {}", report.event_count);
    println!("Policy decisions: {}", report.policy_decision_count);
    println!("Terminal commands: {}", report.terminal_command_count);
    println!(
        "Confirmation responses: {}",
        report.confirmation_response_count
    );
    println!("Blocked commands: {}", report.blocked_command_count);
    println!("Warnings: {}", report.warning_count);
    println!(
        "Failed terminal commands: {}",
        report.failed_terminal_command_count
    );

    println!();
    println!("Evaluations:");
    for evaluation in &report.evaluations {
        println!(
            "{} {} - {}",
            evaluation_status_label(evaluation.passed),
            evaluation.name,
            evaluation.detail
        );
    }

    if !report.policy_decisions.is_empty() {
        println!();
        println!("Policy decisions:");
        for decision in &report.policy_decisions {
            println!(
                "- {} -> {} ({}, rule: {})",
                decision.command, decision.action, decision.risk, decision.rule
            );
        }
    }

    if !report.confirmation_responses.is_empty() {
        println!();
        println!("Confirmation responses:");
        for response in &report.confirmation_responses {
            println!("- {} -> approved={}", response.command, response.approved);
        }
    }

    if !report.terminal_commands.is_empty() {
        println!();
        println!("Terminal commands:");
        for command in &report.terminal_commands {
            println!(
                "- {} -> exit_code={:?}, duration_ms={:?}",
                command.command, command.exit_code, command.duration_ms
            );
        }
    }
}

fn print_ci_report(report: &RunReport) {
    println!("Run: {}", report.metadata.run_id);
    println!("Status: {}", run_status_label(report.metadata.status));
    println!();
    println!("Evaluations:");

    for evaluation in &report.evaluations {
        println!(
            "{} {} - {}",
            evaluation_status_label(evaluation.passed),
            evaluation.name,
            evaluation.detail
        );
    }
}

fn evaluations_passed(evaluations: &[ReportEvaluation]) -> bool {
    evaluations.iter().all(|evaluation| evaluation.passed)
}

fn report_evaluations(
    status: RunStatus,
    blocked_command_count: usize,
    failed_terminal_command_count: usize,
) -> Vec<ReportEvaluation> {
    vec![
        ReportEvaluation {
            name: "no_blocked_commands",
            passed: blocked_command_count == 0,
            detail: format!("blocked_commands={blocked_command_count}"),
        },
        ReportEvaluation {
            name: "no_failed_terminal_commands",
            passed: failed_terminal_command_count == 0,
            detail: format!("failed_terminal_commands={failed_terminal_command_count}"),
        },
        ReportEvaluation {
            name: "run_status_success",
            passed: status == RunStatus::Success,
            detail: format!("status={}", run_status_label(status)),
        },
    ]
}

fn evaluation_status_label(passed: bool) -> &'static str {
    if passed {
        "PASS"
    } else {
        "FAIL"
    }
}

fn execute_run(options: &RunOptions) -> Result<RunSummary, Box<dyn std::error::Error>> {
    let config = read_run_config(&options.config_path)?;

    if config.workflow.steps.is_empty() {
        return Err("workflow.steps must contain at least one command".into());
    }

    let mut writer = TraceWriter::create(&options.runs_dir, env!("CARGO_PKG_VERSION"))?;

    writer.append(TraceEventKind::RunStarted(RunStartedPayload {
        agentharness_version: env!("CARGO_PKG_VERSION").to_owned(),
    }))?;

    let mut step_summaries = Vec::with_capacity(config.workflow.steps.len());
    let mut run_status = RunStatus::Success;

    for command in config.workflow.steps {
        let decision = classify_command(&command);
        writer.append(TraceEventKind::PolicyDecision(policy_decision_payload(
            &command, &decision,
        )))?;

        let step_status = match decision.action {
            PolicyAction::Block => RunStatus::Blocked,
            PolicyAction::RequireConfirmation => {
                let approved = options.yes || prompt_for_confirmation(&command, &decision);

                writer.append(TraceEventKind::ConfirmationResponse(
                    ConfirmationResponsePayload {
                        command: command.clone(),
                        approved,
                        responder: None,
                    },
                ))?;

                if approved {
                    execute_and_trace_command(&mut writer, &command)?
                } else {
                    RunStatus::Blocked
                }
            }
            PolicyAction::Allow | PolicyAction::Warn => {
                execute_and_trace_command(&mut writer, &command)?
            }
        };

        step_summaries.push(StepSummary {
            command,
            decision: decision.action.to_string().to_lowercase(),
            status: run_status_label(step_status),
        });

        if step_status != RunStatus::Success {
            run_status = step_status;
            break;
        }
    }

    writer.append(TraceEventKind::RunFinished(RunFinishedPayload {
        status: run_status,
    }))?;
    writer.finish(run_status)?;

    Ok(RunSummary {
        run_dir: writer.run_dir().to_path_buf(),
        status: run_status_label(run_status),
        steps: step_summaries,
    })
}

fn read_run_config(path: &PathBuf) -> Result<RunConfig, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&contents)?)
}

fn prompt_for_confirmation(command: &str, decision: &PolicyDecision) -> bool {
    println!();
    println!("This command requires confirmation:");
    println!("  Command: {command}");
    println!("  Risk: {}", decision.risk);
    println!("  Reason: {}", decision.reason);
    print!("Proceed? [y/N]: ");
    let _ = io::stdout().flush();

    let mut input = String::new();

    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    parse_confirmation_response(&input)
}

fn parse_confirmation_response(input: &str) -> bool {
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
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
    println!("  agentharness report <runs-dir-or-run-dir>");
    println!("  agentharness ci <runs-dir-or-run-dir>");
    println!("  agentharness policy check \"<command>\"");
    println!("  agentharness trace demo [--runs-dir <path>] [--command \"<cmd>\"]");
    println!();
    println!("Examples:");
    println!("  agentharness run examples/risky-command.yaml");
    println!("  agentharness run examples/multi-step-command.yaml");
    println!("  agentharness report runs");
    println!("  agentharness ci runs");
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
    fn parses_report_options() {
        let options = parse_report_options(&["runs".to_owned()]).expect("report parses");

        assert_eq!(options.path, PathBuf::from("runs"));
    }

    #[test]
    fn rejects_report_options_with_extra_arguments() {
        assert!(parse_report_options(&["runs".to_owned(), "extra".to_owned()]).is_err());
    }

    #[test]
    fn report_evaluations_pass_for_successful_clean_run() {
        let evaluations = report_evaluations(RunStatus::Success, 0, 0);

        assert_eq!(
            evaluations,
            vec![
                ReportEvaluation {
                    name: "no_blocked_commands",
                    passed: true,
                    detail: "blocked_commands=0".to_owned(),
                },
                ReportEvaluation {
                    name: "no_failed_terminal_commands",
                    passed: true,
                    detail: "failed_terminal_commands=0".to_owned(),
                },
                ReportEvaluation {
                    name: "run_status_success",
                    passed: true,
                    detail: "status=success".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn report_evaluations_fail_for_blocked_or_failed_run() {
        let evaluations = report_evaluations(RunStatus::Blocked, 1, 0);

        assert_eq!(evaluations[0].name, "no_blocked_commands");
        assert!(!evaluations[0].passed);
        assert_eq!(evaluations[1].name, "no_failed_terminal_commands");
        assert!(evaluations[1].passed);
        assert_eq!(evaluations[2].name, "run_status_success");
        assert!(!evaluations[2].passed);

        let evaluations = report_evaluations(RunStatus::Failed, 0, 1);
        assert!(evaluations[0].passed);
        assert!(!evaluations[1].passed);
        assert!(!evaluations[2].passed);
    }

    #[test]
    fn parse_confirmation_response_accepts_y_variants() {
        assert!(parse_confirmation_response("y"));
        assert!(parse_confirmation_response("Y\n"));
        assert!(parse_confirmation_response("yes"));
        assert!(parse_confirmation_response("  YES  "));
    }

    #[test]
    fn parse_confirmation_response_rejects_everything_else() {
        assert!(!parse_confirmation_response("n"));
        assert!(!parse_confirmation_response("no"));
        assert!(!parse_confirmation_response(""));
        assert!(!parse_confirmation_response("\n"));
        assert!(!parse_confirmation_response("sure"));
    }

    #[test]
    fn evaluations_passed_is_true_when_all_pass() {
        let evaluations = report_evaluations(RunStatus::Success, 0, 0);

        assert!(evaluations_passed(&evaluations));
    }

    #[test]
    fn evaluations_passed_is_false_when_any_fail() {
        let evaluations = report_evaluations(RunStatus::Blocked, 1, 0);

        assert!(!evaluations_passed(&evaluations));
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

    #[test]
    fn execute_run_runs_every_step_when_all_succeed() {
        let (runs_dir, config_path) = test_run_scenario(
            "multi_step_success",
            "id: t\nworkflow:\n  steps:\n    - \"echo one\"\n    - \"echo two\"\n",
        );

        let summary = execute_run(&RunOptions {
            config_path,
            runs_dir: runs_dir.clone(),
            yes: false,
        })
        .expect("run should complete");

        assert_eq!(summary.status, "success");
        assert_eq!(summary.steps.len(), 2);
        assert!(summary.steps.iter().all(|step| step.status == "success"));

        fs::remove_dir_all(runs_dir.parent().unwrap_or(&runs_dir)).ok();
    }

    #[test]
    fn execute_run_stops_after_first_failed_step() {
        let (runs_dir, config_path) = test_run_scenario(
            "multi_step_stop_on_failure",
            "id: t\nworkflow:\n  steps:\n    - \"echo one\"\n    - \"exit 1\"\n    - \"echo three\"\n",
        );

        let summary = execute_run(&RunOptions {
            config_path,
            runs_dir: runs_dir.clone(),
            yes: false,
        })
        .expect("run should complete");

        assert_eq!(summary.status, "failed");
        assert_eq!(summary.steps.len(), 2);
        assert_eq!(summary.steps[0].status, "success");
        assert_eq!(summary.steps[1].status, "failed");

        fs::remove_dir_all(runs_dir.parent().unwrap_or(&runs_dir)).ok();
    }

    #[test]
    fn execute_run_stops_after_first_blocked_step() {
        let (runs_dir, config_path) = test_run_scenario(
            "multi_step_stop_on_block",
            "id: t\nworkflow:\n  steps:\n    - \"echo one\"\n    - \"rm -rf /\"\n    - \"echo three\"\n",
        );

        let summary = execute_run(&RunOptions {
            config_path,
            runs_dir: runs_dir.clone(),
            yes: false,
        })
        .expect("run should complete");

        assert_eq!(summary.status, "blocked");
        assert_eq!(summary.steps.len(), 2);
        assert_eq!(summary.steps[1].decision, "block");

        fs::remove_dir_all(runs_dir.parent().unwrap_or(&runs_dir)).ok();
    }

    #[test]
    fn execute_run_rejects_empty_steps() {
        let (runs_dir, config_path) =
            test_run_scenario("multi_step_empty", "id: t\nworkflow:\n  steps: []\n");

        let error = execute_run(&RunOptions {
            config_path,
            runs_dir: runs_dir.clone(),
            yes: false,
        })
        .expect_err("empty steps should be rejected");

        assert!(error.to_string().contains("workflow.steps"));

        fs::remove_dir_all(runs_dir.parent().unwrap_or(&runs_dir)).ok();
    }

    fn test_run_scenario(name: &str, config_yaml: &str) -> (PathBuf, PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let scenario_dir = std::env::temp_dir().join(format!("agentharness_cli_{name}_{nanos}"));
        fs::create_dir_all(&scenario_dir).expect("scenario dir should be created");

        let config_path = scenario_dir.join("config.yaml");
        fs::write(&config_path, config_yaml).expect("config should be written");

        let runs_dir = scenario_dir.join("runs");

        (runs_dir, config_path)
    }
}
