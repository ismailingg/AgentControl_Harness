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

Deferred trace work:

- span/parent-child nesting
- interactive confirmation prompting
- full shell parsing
- path canonicalization and environment expansion
- secret exfiltration detection
- allowed-directory policy
- generalized compound-command classification
