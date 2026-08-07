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

Deferred trace work (status as of the ADR 003 model/tool call event slice):

- span/parent-child nesting
- `file_change` event type (see ADR 003 rationale for why it's deferred)
- `evaluation` and `suggestion` events
- richer `confirmation_response.responder` metadata
- anything that actually emits `model_call` or `tool_call` events - the schema exists (ADR 003) but no model integration or non-terminal tool integration exists yet in `agentharness run`
- `report`/`ci` counting, printing, or evaluating `model_call`/`tool_call` events - they currently fall through a catch-all match and are silently ignored
- full shell parsing
- path canonicalization and environment expansion
- secret exfiltration detection
- allowed-directory policy
- generalized compound-command classification

Resolved since this section was first written: real command execution and `terminal_command` emission from `agentharness run`, final run status from actual execution outcome, interactive confirmation prompting, and the `model_call`/`tool_call` trace schema (ADR 003) are all implemented.

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
- for `require_confirmation` decisions, prompts interactively on stdin (`Proceed? [y/N]`) unless `--yes` is passed, in which case it auto-approves
- writes `confirmation_response` with the real approve/decline outcome
- executes `allow`, `warn`, and approved `require_confirmation` commands
- writes `terminal_command` with exit code, duration, and stdout/stderr excerpts
- writes `run_finished`
- updates `metadata.json`

Temporary v1 constraints:

- only one command per config
- command strings execute through the platform shell (`cmd /C` on Windows, `sh -c` elsewhere)
- `--yes` bypasses the prompt entirely; there is no way yet to require the prompt even when `--yes` is set (e.g. for a hardened CI mode)
- `confirmation_response.responder` is always `null`; no richer responder metadata (username, source) yet
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

Report v1 also prints deterministic evaluations:

- `no_blocked_commands`
- `no_failed_terminal_commands`
- `run_status_success`

Temporary v1 constraints:

- terminal text output only
- no JSON report output yet
- no numeric scoring
- no comparison
- no aggregation across multiple runs

## CI v1 Slice

The first CI gate command is:

```text
agentharness ci <runs-dir-or-run-dir>
```

It takes the same argument shape as `report` (a `runs/` dir with `latest.txt`, or a direct run dir) and reuses the exact same report-building and evaluation logic. It does not add any new evaluations or trace data — it is purely a pass/fail gate on top of the three deterministic evaluations `report` already computes (`no_blocked_commands`, `no_failed_terminal_commands`, `run_status_success`).

Output is condensed compared to `report`: run ID, status, and the PASS/FAIL evaluation lines only (no policy/terminal-command/confirmation detail dump).

Exit codes:

- `0` - every evaluation passed
- `1` - an evaluation failed, or the run/report could not be read at all (missing directory, corrupt JSON, etc.)
- `64` - usage error (missing argument)

Temporary v1 constraints:

- no configurable fail-if thresholds (see project plan §7.11 `CI Gates` for the eventual richer design); v1 only gates on the fixed set of deterministic evaluations `report` already produces
- no JSON output

## Interactive Confirmation Prompting Slice

`agentharness run` now prompts on stdin for `require_confirmation` decisions instead of always auto-declining:

```text
This command requires confirmation:
  Command: git clean -fd
  Risk: HIGH
  Reason: git clean can remove untracked files permanently
Proceed? [y/N]:
```

`--yes` still bypasses the prompt entirely and auto-approves, for scripted/non-interactive use. The `confirmation_response` event already existed in the schema (ADR 002) and is unchanged in shape - this slice only changes how the `approved` field gets decided.

If stdin can't be read (EOF, closed pipe), the command is treated as declined rather than erroring.

Temporary constraints:

- `confirmation_response.responder` is still always `null`; the prompt does not capture who answered
- no way to force the prompt even when `--yes` is passed (e.g. a stricter CI mode that always wants an explicit answer)

## Model Call / Tool Call Event Types Slice (ADR 003)

`agentharness-trace` now has two new event types, `model_call` and `tool_call`, alongside the six from ADR 002. See `docs/adr-003-model-and-tool-call-events.md` for the full design rationale.

`model_call` is fully typed for cost/token tracking:

```text
model, tokens_in, tokens_out, cost_usd, duration_ms, prompt_excerpt, response_excerpt
```

`tool_call` is a smaller generic shape for everything else (file ops, API calls, custom tools):

```text
tool, summary, duration_ms, status (success | error), detail_excerpt
```

This slice is schema-only. Nothing in `agentharness run` emits either event type yet, and `report`/`ci` do not count, print, or evaluate them - they currently fall through a catch-all match in `build_report` and are silently ignored. The next consumer of this schema will be whatever eventually integrates a real model call or non-terminal tool into `agentharness run`.
