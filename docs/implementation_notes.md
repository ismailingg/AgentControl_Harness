# Implementation Notes

## Current Approved Architecture

The initial approved direction is:

- Rust workspace
- CLI-first
- JSONL trace protocol later
- deterministic evaluators before LLM-as-judge
- command safety first
- local run directories before database
- framework-agnostic adapters

## First Vertical Slice

The first slice implements:

```text
agentharness policy check "<command>"
```

This command classifies a terminal command as:

- `allow`
- `warn`
- `require_confirmation`
- `block`

This is intentionally deterministic and dependency-light. The goal is to create the safety kernel before introducing model integrations, trace storage, dashboards, or SDK adapters.

## Why Command Safety First

Agentic coding tools operate directly against a developer's machine, repository, terminal, and sometimes credentials. A command firewall gives AgentHarness a concrete control point before expensive tracing and evaluation layers are added.

## Next Architecture Decision

Trace Layer v1 is now approved in:

```text
docs/adr-002-trace-layer-v1.md
```

Approved decisions:

- flat JSONL events for v1
- common event envelope with `type`, `run_id`, `seq`, `timestamp`, and `payload`
- `runs/latest.txt` instead of a symlink
- per-run `metadata.json` and `events.jsonl`
- raw commands in traces
- pure `agentharness-policy` crate with no trace I/O
- no trace emission from standalone `agentharness policy check`
- synchronous per-event JSONL writes

The next implementation slice is the `agentharness-trace` crate:

- trace event types
- JSON serialization and deserialization
- JSONL append writer
- run directory creation
- metadata writing
- latest-run pointer updates

## Trace Demo Slice

The CLI includes a small proof-of-plumbing command:

```text
agentharness trace demo [--runs-dir <path>] [--command "<cmd>"]
```

This command creates a run directory, writes `run_started`, calls the real policy classifier, writes `policy_decision`, and finishes metadata.

It intentionally does not execute commands and does not emit `terminal_command`.

Temporary status mapping for this demo:

- `allow` and `warn` -> `success`
- `require_confirmation` and `block` -> `blocked`

For `trace demo`, `success` means "classified as non-blocking", not "executed successfully". This must be revisited when `agentharness run` actually executes commands.

Deferred trace work:

- span/parent-child nesting
- interactive confirmation prompting
- real command execution and `terminal_command` emission from `agentharness run`
- final run status based on actual command execution outcome
- full shell parsing
- path canonicalization and environment expansion
- secret exfiltration detection
- allowed-directory policy
- generalized compound-command classification

## Run v1 Slice

The first real run command is:

```text
agentharness run <config-file> [--runs-dir <path>] [--yes]
```

The v1 config shape is intentionally minimal:

```yaml
id: risky_command_demo

workflow:
  command: "cargo test"
```

Run v1 behavior:

- creates a trace run
- writes `run_started`
- classifies `workflow.command`
- writes `policy_decision`
- blocks `block` decisions without execution
- blocks `require_confirmation` decisions unless `--yes` is passed
- writes `confirmation_response` for `require_confirmation`
- executes `allow`, `warn`, and `require_confirmation --yes` commands
- writes `terminal_command` with exit code, duration, and stdout/stderr excerpts
- writes `run_finished`
- updates `metadata.json`

Temporary v1 constraints:

- only one command per config
- command strings execute through the platform shell (`cmd /C` on Windows, `sh -c` elsewhere)
- no interactive prompt yet; `--yes` is the temporary approval mechanism
- no model/tool/file/evaluation/suggestion events yet
- no working-directory config yet

## Report v1 Slice

The first trace reader command is:

```text
agentharness report <runs-dir-or-run-dir>
```

If the path contains `latest.txt`, report v1 resolves it to the latest run directory. Otherwise, it treats the path as a direct run directory.

Report v1 reads:

- `metadata.json`
- `events.jsonl`

It prints:

- run ID, path, status, start/end timestamps
- event count
- policy decision count
- terminal command count
- confirmation response count
- blocked command count
- warning count
- failed terminal command count
- compact policy/confirmation/terminal command details

Temporary v1 constraints:

- terminal text output only
- no JSON report output yet
- no scoring
- no comparison
- no CI gate behavior
- no aggregation across multiple runs
