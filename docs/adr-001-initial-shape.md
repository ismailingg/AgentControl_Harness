# ADR 001: Initial Shape

## Status

Approved.

## Decision

AgentHarness starts as a Rust workspace with a CLI-first interface.

The approved initial architecture is:

- Rust workspace with multiple crates.
- CLI-first instead of dashboard-first.
- JSONL trace protocol in a later slice.
- Deterministic evaluators before LLM-as-judge.
- Command safety as the first feature.
- Local run directories before a database.
- Framework-agnostic adapters.

## Rationale

AgentHarness is intended to become developer infrastructure for agentic coding workflows. The core needs to be reliable, local-first, inspectable, and suitable for CI.

Starting with a Rust CLI gives us a practical control point before adding dashboards, SDKs, IDE extensions, or provider-specific integrations.

## First Implementation Slice

The first approved slice is:

```text
agentharness policy check "<command>"
```

This classifies a command as:

- `ALLOW`
- `WARN`
- `REQUIRE_CONFIRMATION`
- `BLOCK`

The goal is to establish the safety kernel before tracing, reporting, and model routing are added.

