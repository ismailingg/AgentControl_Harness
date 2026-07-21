# ADR 002: Trace Layer v1 Design

## Status

Approved.

## Decision

AgentHarness adopts a flat, JSONL-based trace format for v1.

The approved v1 design is:

- Flat JSONL events with no span or parent-child nesting yet.
- A common event envelope containing `type`, `run_id`, `seq`, `timestamp`, and a type-specific `payload` object.
- A per-run `seq` field, starting at 1 and scoped to one `run_id`, for unambiguous ordering.
- Raw commands recorded in traces exactly as issued. Normalization remains internal to the policy matcher.
- `runs/latest.txt`, a plain text file naming the most recent run, instead of a `runs/latest` symlink.
- One run directory per run, containing `metadata.json` and `events.jsonl`.
- `metadata.json` stores run-level facts. `events.jsonl` stores the append-only chronological event stream.
- `policy_decision` and `terminal_command` are distinct event types. A blocked command produces a `policy_decision` event but no `terminal_command` event because it never executes.
- `confirmation_response` is reserved as an event type now, but its implementation waits until interactive confirmation prompting exists.
- `agentharness-policy` remains pure: no file I/O, no run directories, and no trace writing.
- A future runner/core layer calls `classify_command`, receives a `PolicyDecision`, and writes trace events through `agentharness-trace`.
- The standalone `agentharness policy check "<command>"` command remains side-effect free and does not emit trace events.
- JSONL event writes are synchronous per event so traces survive crashes after blocked or failed commands.

## Rationale

Later features such as `report`, `compare`, and CI gates need a reliable, ordered record of what happened during a run. This ADR locks down the event shape and local file layout before those readers depend on it.

Span nesting is deliberately deferred. It will be useful once AgentHarness has real nested runs, tool calls, and sub-steps, but v1 can get reliable ordering from a flat event list plus `seq`.

The `payload` envelope keeps common event metadata separate from event-specific fields. This maps cleanly to Rust enum-style modeling and avoids every reader dealing with a loosely typed top-level JSON object.

Recording raw commands preserves the audit trail. The policy engine may normalize commands for matching, but the trace should show what the agent actually attempted.

Using `latest.txt` instead of a symlink avoids Windows permission problems. A plain text pointer works without Developer Mode or administrator privileges.

Keeping `agentharness-policy` pure preserves simple deterministic testing. Policy classification remains a function call with no hidden I/O or run-directory setup.

Synchronous writes are chosen because trace data is most valuable when something goes wrong. If a command is blocked or the process crashes, the latest event should already be on disk.

## Event Envelope

Example `policy_decision` event:

```json
{
  "type": "policy_decision",
  "run_id": "run_20260721_120000_a1b2",
  "seq": 2,
  "timestamp": "2026-07-21T12:00:00Z",
  "payload": {
    "command": "sudo RM -Rf /",
    "action": "block",
    "risk": "critical",
    "rule": "destructive-root-delete",
    "reason": "command attempts a destructive delete against a root/system path"
  }
}
```

`seq` starts at 1 for each run and increases by 1 for every event appended in that run.

## Run Directory Layout

```text
runs/
  latest.txt
  run_20260721_120000_a1b2/
    metadata.json
    events.jsonl
```

`latest.txt` contains only the latest run directory name:

```text
run_20260721_120000_a1b2
```

`metadata.json` holds run-level facts:

```json
{
  "run_id": "run_20260721_120000_a1b2",
  "started_at": "2026-07-21T12:00:00Z",
  "finished_at": "2026-07-21T12:00:42Z",
  "status": "failed",
  "agentharness_version": "0.1.0"
}
```

`status` is one of:

```text
running
success
failed
blocked
error
```

`events.jsonl` holds one JSON object per line.

## v1 Event Types

The initial event types are:

```text
run_started
policy_decision
terminal_command
confirmation_response
run_finished
error
```

`confirmation_response` is reserved now so the schema has a clear place for future human approval or denial records.

Expected command flow:

```text
ALLOW
  policy_decision
  terminal_command

WARN
  policy_decision
  terminal_command

REQUIRE_CONFIRMATION approved
  policy_decision
  confirmation_response
  terminal_command

REQUIRE_CONFIRMATION declined
  policy_decision
  confirmation_response

BLOCK
  policy_decision
```

## Crate Responsibility

```text
agentharness-policy
  Pure command classification.
  classify_command() takes a command string and returns a PolicyDecision.
  No file I/O, no run directories, no trace writing.

agentharness-trace
  Event schema.
  JSONL writer.
  Run directory creation.
  metadata.json writing.
  latest.txt updates.

future runner / agentharness-core
  Calls classify_command().
  Receives PolicyDecision.
  Writes policy_decision events through agentharness-trace.
  Decides whether to execute commands.
```

`agentharness policy check "<command>"` remains a standalone diagnostic command with no trace output.

## Deferred

These items are explicitly deferred:

- Span and parent-child nesting with `span_id` and `parent_span_id`.
- Interactive confirmation prompting and actual `confirmation_response` emission.
- Full shell parsing.
- Path canonicalization, symlink resolution, and environment variable expansion.
- Secret exfiltration detection.
- Allowed-directory policy.
- Generalized compound-command classification.
- Moving policy logic into any I/O-owning crate.

## First Implementation Slice

The first implementation slice for this ADR is a new `agentharness-trace` crate that provides:

- Trace event types matching the envelope above.
- JSON serialization and deserialization.
- Synchronous JSONL append writer.
- Run directory creation.
- `metadata.json` writing.
- `latest.txt` updates.

This slice does not implement `agentharness run`. The future runner/core layer will be the first caller of `agentharness-trace`.
