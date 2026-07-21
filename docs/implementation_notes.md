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

Before the next substantial implementation step, decide how traces should be represented:

- event enum shape
- JSONL file layout
- run directory naming
- span parent-child model
- whether policy decisions are emitted as trace events immediately

