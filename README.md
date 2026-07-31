# AgentHarness

AgentHarness is a Rust-based control and evaluation layer for agentic developer workflows.

The first implementation slice focuses on command safety: classifying terminal commands before an agent runs them.

## First Command

```bash
cargo run -p agentharness-cli -- policy check "rm -rf /"
```

Expected result:

```text
Decision: BLOCK
Risk: CRITICAL
Rule: destructive-root-delete
Reason: command attempts a destructive delete against a root/system path
```

## First Run Command

```bash
cargo run -p agentharness-cli -- run examples/risky-command.yaml
```

This creates a local trace run, classifies the configured command, blocks unsafe commands before execution, and records events in:

```text
runs/
  latest.txt
  run_.../
    metadata.json
    events.jsonl
```

For safe manual testing:

```bash
cargo run -p agentharness-cli -- run examples/allow-command.yaml
```

To summarize the latest run in a runs directory:

```bash
cargo run -p agentharness-cli -- report runs
```

You can also report a specific run directory:

```bash
cargo run -p agentharness-cli -- report runs/run_...
```

Run v1 supports one configured command:

```yaml
workflow:
  command: "cargo test"
```

`REQUIRE_CONFIRMATION` commands do not execute unless `--yes` is passed.

## Workspace

```text
crates/
  agentharness-cli/
  agentharness-policy/
  agentharness-trace/
examples/
  allow-command.yaml
  confirmation-command.yaml
  policy_cases.txt
  risky-command.yaml
docs/
  implementation_notes.md
```

## Current Scope

- Rust workspace
- CLI-first interface
- deterministic command safety classifier
- local JSONL trace writer
- single-command `agentharness run` v1
- examples for risky and allowed commands
- unit tests for policy classification
