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

## Workspace

```text
crates/
  agentharness-cli/
  agentharness-policy/
examples/
  policy_cases.txt
docs/
  implementation_notes.md
```

## Current Scope

- Rust workspace
- CLI-first interface
- deterministic command safety classifier
- examples for risky and allowed commands
- unit tests for policy classification

