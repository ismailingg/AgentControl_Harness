# ADR 003: Model Call and Tool Call Trace Events

## Status

Approved.

## Decision

AgentHarness adds two new trace event types to the v1 schema established in ADR 002: `model_call` and `tool_call`.

`file_change` is explicitly deferred rather than added alongside them. `evaluation` and `suggestion` events (persisting report-time computed data back into the trace) are also deferred as a separate concern.

## Rationale

The trace schema from ADR 002 only describes terminal commands. To be useful for actual agent workflows (per the project's core pitch: tracing model calls, tool calls, and terminal commands), it needs to describe those too.

Rather than design three new event types at once, this ADR splits them by whether a concrete near-term consumer exists:

- `model_call` gets a fully typed payload (tokens, cost, duration) because two features already on the roadmap need it: loop detection (`§7.5`, detecting repeated model calls with similar content) and cost spike detection (`§7.6`, summing and thresholding `cost_usd`/token counts). Both need numeric, structured fields — a free-text summary would force parsing numbers back out of prose later, which ADR 002 already rejected once for the same reason (see its rationale on the payload envelope).
- `tool_call` gets a smaller, more generic payload for everything else (file operations, API calls, custom tools) where no near-term evaluator depends on specific structured fields yet. Loop detection also wants "same tool call repeated with same arguments"; the `summary` field is enough for that today via string comparison, and can be made more structured later once a real use case demands it.
- `file_change` is deferred entirely. Nothing in the near-term roadmap (Phase 4 evaluators: loop detector, cost limits, step limits, required-command check, final-status check, tool contract check) consumes file-change data. The only place file tracking appears is the longer-term `§7.7` code-gen evaluation vision, which is not part of the immediate roadmap. Designing its fields now would be speculative.

## Event Payloads

`model_call`:

```json
{
  "type": "model_call",
  "run_id": "run_20260805_120000_a1b2",
  "seq": 4,
  "timestamp": "2026-08-05T12:00:00Z",
  "payload": {
    "model": "claude",
    "tokens_in": 1800,
    "tokens_out": 400,
    "cost_usd": 0.018,
    "duration_ms": 2200,
    "prompt_excerpt": "fix the failing test",
    "response_excerpt": "updated assertion in test_foo"
  }
}
```

All fields except `model` are optional, since not every caller will have cost/token data available.

`tool_call`:

```json
{
  "type": "tool_call",
  "run_id": "run_20260805_120000_a1b2",
  "seq": 5,
  "timestamp": "2026-08-05T12:00:01Z",
  "payload": {
    "tool": "file.write",
    "summary": "wrote src/lib.rs",
    "duration_ms": 12,
    "status": "success",
    "detail_excerpt": null
  }
}
```

`tool` is a free-form namespaced string (e.g. `"terminal"`, `"model.claude"`, `"file.write"`) rather than an enum, so new tool kinds don't require a schema change. `status` is a typed `success`/`error` enum, since it is intrinsic to the trace crate itself (unlike `policy_decision`'s string-typed `action`/`risk` fields, which stay strings because the trace crate deliberately has no dependency on `agentharness-policy`).

## Consequences

`cost_usd: Option<f64>` means `ModelCallPayload`, and therefore `TraceEventKind` and `TraceEvent`, can no longer derive `Eq` (only `PartialEq`) — `f64` does not implement `Eq` because of `NaN`. Nothing in the codebase relied on `Eq` specifically (only `PartialEq`, via `assert_eq!` in tests), so this has no functional impact.

`prompt_excerpt`, `response_excerpt`, and `detail_excerpt` follow the same truncation convention as `terminal_command`'s `stdout_excerpt`/`stderr_excerpt`: truncation is the caller's responsibility (e.g. `OUTPUT_EXCERPT_LIMIT` in `agentharness-cli`), not the trace crate's. There is no caller yet — see below.

## Deferred

- `file_change` event type.
- `evaluation` and `suggestion` events.
- Anything that actually emits `model_call` or `tool_call` events. This ADR only adds the schema; there is no model integration or non-terminal tool integration in `agentharness run` yet, so these event types are reserved for now, the same way `confirmation_response` was reserved by ADR 002 before interactive confirmation prompting existed.
- `report`/`ci` counting, printing, or evaluating these new event types. They currently fall through `build_report`'s catch-all match arm and are silently ignored.

## First Implementation Slice

Add to `agentharness-trace`:

- `ModelCallPayload` and `ToolCallPayload` structs.
- `ToolCallStatus` enum (`success` / `error`).
- `TraceEventKind::ModelCall` and `TraceEventKind::ToolCall` variants.
- Corresponding `TraceEventType` variants and `event_type()` mapping.
- Serialization/deserialization tests following the existing envelope test pattern.
