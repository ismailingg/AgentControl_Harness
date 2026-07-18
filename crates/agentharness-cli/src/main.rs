use agentharness_policy::{classify_command, PolicyAction};
use std::env;
use std::process::ExitCode;

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
        _ => {
            eprintln!("Unknown command.");
            print_help();
            ExitCode::from(64)
        }
    }
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

fn print_help() {
    println!("AgentHarness");
    println!();
    println!("Usage:");
    println!("  agentharness policy check \"<command>\"");
    println!();
    println!("Examples:");
    println!("  agentharness policy check \"cargo test\"");
    println!("  agentharness policy check \"rm -rf /\"");
}
