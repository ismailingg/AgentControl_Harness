# AgentHarness Project Plan

## 1. Project Summary

AgentHarness is a Rust-based reliability, safety, and optimization harness for agentic developer workflows.

It is designed to plug into LLM agents, coding agents, IDE tools, CI pipelines, and custom automation systems. Its purpose is to observe agent behavior, evaluate quality, block risky actions, compare prompt/model/config changes, and help developers make agentic workflows safer and more reliable.

In short:

> AgentHarness is a control and evaluation layer for modern agentic development workflows.

It should feel like:

```text
pytest + OpenTelemetry + policy firewall + prompt regression testing + CI gate
```

But focused on LLM agents, coding agents, and tool-using workflows.

## 2. Product Positioning

AgentHarness is not just another chatbot, agent demo, or tracing dashboard.

It is developer infrastructure.

The core pitch:

> AgentHarness is a Rust-based package, CLI, and plugin system that traces prompts, context, model calls, tool calls, terminal commands, costs, and outputs; blocks unsafe actions; detects loops and hallucinations; compares prompt/model/config versions; and gates agentic workflows in CI.

The goal is to become a normal part of a modern developer's toolchain, especially for teams using OpenAI, Claude, Cursor, Codex-style coding agents, LangChain, LangGraph, custom internal agents, or AI-powered automation.

## 3. Core Concept

AgentHarness sits between the agent and the environment.

```text
Agent / LLM / IDE / Workflow
        |
        v
AgentHarness Rust Runtime
        |
        v
Tools, terminal, files, APIs, model providers, CI
```

The harness has four major responsibilities:

1. Observe agent behavior.
2. Evaluate quality and reliability.
3. Control risky or wasteful actions.
4. Optimize future runs.

## 4. Target Users

Primary users:

- Developers using AI coding agents.
- Teams building internal LLM workflows.
- AI engineers testing prompts and tools.
- DevOps teams adding AI workflow checks to CI.
- Platform teams building agent infrastructure.

Secondary users:

- Researchers evaluating agent behavior.
- Startups building AI copilots.
- Engineering managers monitoring AI-assisted development reliability.

## 5. Key Use Cases

### 5.1 Coding Agent Supervision

A developer runs a coding agent to fix bugs, write tests, or refactor code. AgentHarness records every step, checks terminal commands, detects unsafe actions, evaluates the final diff, and verifies whether tests passed.

Example:

```bash
agentharness run examples/codegen.yaml
agentharness report runs/latest
```

### 5.2 Prompt Regression Testing

A team changes an agent's system prompt, model, temperature, context strategy, or tool schema. AgentHarness runs the same test suite and compares old vs new results.

Example:

```bash
agentharness compare runs/baseline runs/latest
```

### 5.3 CI Quality Gate

A pull request changes prompts or agent configuration. AgentHarness runs in GitHub Actions and fails the PR if quality drops, cost spikes, loops increase, or blocked commands are attempted.

Example:

```bash
agentharness ci --config agentharness.yaml
```

### 5.4 Command Safety Firewall

Before an agent runs terminal commands, AgentHarness classifies them as allowed, warning-worthy, confirmation-required, or blocked.

Examples:

```text
ALLOW: cargo test
ALLOW: npm run lint
WARN: npm install
REQUIRE_CONFIRMATION: git clean -fd
BLOCK: rm -rf /
BLOCK: git reset --hard
BLOCK: del /s C:\
```

### 5.5 Model Routing

AgentHarness routes tasks to models based on complexity, cost, and risk.

Example:

```text
Simple read-only task -> cheaper model
Code edit task -> balanced model
Security-sensitive task -> strongest model plus stricter policy
Repeated failure -> escalate model
```

## 6. Product Form

AgentHarness should ship in multiple forms over time.

### 6.1 Rust CLI

The first and most important interface.

```bash
agentharness run
agentharness trace
agentharness report
agentharness compare
agentharness policy check
agentharness ci
```

### 6.2 Rust Library

For direct embedding into Rust-based tools.

Example:

```rust
use agentharness::{Harness, Policy};
```

### 6.3 Python Adapter

For Python agents, LangChain, LangGraph, OpenAI SDK workflows, and internal scripts.

Example:

```python
from agentharness import trace, tool_call
```

### 6.4 TypeScript Adapter

For Node.js agents, frontend-heavy AI tools, and JavaScript automation.

Example:

```ts
import { trace, toolCall } from "@agentharness/sdk";
```

### 6.5 GitHub Action

For CI integration.

Example:

```yaml
- uses: agentharness/agentharness-action@v1
  with:
    config: agentharness.yaml
```

### 6.6 IDE Extension

Later extension targets:

- VS Code
- Cursor
- JetBrains IDEs
- Codex-like local coding environments

The IDE extension should surface:

- blocked commands
- risky file edits
- trace timeline
- cost warnings
- loop warnings
- final run score

## 7. Main Features

### 7.1 Trace Capture

AgentHarness records every meaningful event in a run.

Captured data:

- run start and end
- model calls
- prompts
- system messages
- user inputs
- retrieved context
- tool calls
- terminal commands
- file reads
- file writes
- file diffs
- retries
- errors
- latency
- token usage
- estimated cost
- final answer
- generated artifacts

Approved v1 storage format:

```text
runs/latest.txt
runs/run_20260721_120000_a1b2/events.jsonl
```

Trace Layer v1 is governed by ADR 002. The v1 event protocol is flat JSONL with a common envelope:

```json
{
  "type": "policy_decision",
  "run_id": "run_20260721_120000_a1b2",
  "seq": 2,
  "timestamp": "2026-07-21T12:00:00Z",
  "payload": {
    "command": "rm -rf /",
    "action": "block",
    "risk": "critical",
    "rule": "destructive-root-delete",
    "reason": "command attempts a destructive delete against a root/system path"
  }
}
```

Trace Layer v1 decisions:

- Flat events only; span nesting is deferred.
- Every event has `type`, `run_id`, `seq`, `timestamp`, and `payload`.
- `seq` is per-run and starts at 1.
- Commands are recorded raw, as issued by the agent.
- `runs/latest.txt` is used instead of a symlink for Windows compatibility.
- Each run directory contains `metadata.json` and `events.jsonl`.
- `agentharness-policy` remains pure and does not write traces.
- `agentharness policy check "<command>"` remains side-effect free.
- Trace writes are synchronous per event.

Trace Layer v1 event types:

```text
run_started
policy_decision
terminal_command
confirmation_response
run_finished
error
```

Trace work explicitly deferred:

- `span_id` and `parent_span_id`.
- Interactive confirmation prompting and actual `confirmation_response` emission.
- Full shell parsing.
- Path canonicalization, symlink resolution, and environment variable expansion.
- Secret exfiltration detection.
- Allowed-directory policy.
- Generalized compound-command classification.

### 7.2 Command Safety Firewall

The command safety firewall checks terminal commands before execution.

Possible decisions:

```text
ALLOW
WARN
REQUIRE_CONFIRMATION
BLOCK
```

Checks should include:

- destructive file operations
- recursive deletes
- dangerous Git commands
- suspicious shell piping
- secret exfiltration attempts
- unsafe package scripts
- commands outside allowed directories
- commands requiring elevated permissions
- mass file modification commands

Initial examples:

```text
BLOCK: rm -rf /
BLOCK: git reset --hard
BLOCK: del /s C:\
BLOCK: sudo rm -rf
BLOCK: curl $SECRET
WARN: npm install
WARN: pip install
REQUIRE_CONFIRMATION: git clean -fd
```

### 7.3 Tool-Call Validation

Tool calls should be checked against declared contracts.

Validation examples:

- required argument missing
- wrong argument type
- tool not allowed
- tool used at the wrong phase
- destructive action without confirmation
- repeated failed tool call
- tool called with hallucinated resource ID
- terminal used when read-only mode is active

### 7.4 Prompt and Config Regression Testing

AgentHarness should compare prompt, model, retrieval, and tool configuration versions.

Example output:

```text
Baseline vs Latest

Success rate:        72% -> 84%
Avg cost/run:        $0.041 -> $0.035
Loop failures:       8% -> 3%
Bad tool calls:      11% -> 5%
Blocked commands:    0 -> 0
Avg latency:         18.2s -> 14.9s
```

### 7.5 Loop Detection

Detect agent behavior that indicates lack of progress.

Signals:

- same command repeated multiple times
- same tool call repeated with same arguments
- same file read repeatedly
- repeated failed command without code changes
- repeated planning with no execution
- token usage increasing without new artifacts
- repeated model calls with similar content

### 7.6 Cost Spike Detection

Track cost and detect budget issues.

Checks:

- max cost per run
- max cost per task
- max tokens per run
- max model calls
- expensive model used for simple task
- cost increase compared to baseline

### 7.7 Code Generation Evaluation

For coding agents, evaluate concrete development outcomes.

Checks:

- tests passed
- lint passed
- typecheck passed
- expected files changed
- unexpected files avoided
- generated/vendor files avoided
- secrets avoided
- final answer matches actual evidence
- no harmful commands attempted
- no false success claim

### 7.8 Hallucination Detection

Start with deterministic suspicion signals.

Examples:

- final answer claims tests passed but no test command ran
- final answer references files that do not exist
- final answer cites tool output that was never observed
- final answer says a bug was fixed but no relevant files changed
- retrieved context does not support the answer

Later, add LLM-as-judge for fuzzy cases.

### 7.9 Suggestion Engine

After a run, AgentHarness should suggest improvements.

Example suggestions:

```text
- The agent called terminal before inspecting the project. Add a repo-scan step to the system prompt.
- The agent repeated `cargo test` 4 times with no code changes. Add a loop cutoff after repeated identical failures.
- Tool argument errors came from missing `working_directory`. Tighten the tool schema.
- This task was low complexity. Use a cheaper model for similar future runs.
```

### 7.10 Model Router

Route tasks based on difficulty, safety, and historical performance.

Initial routing can be rule-based.

Example:

```yaml
routing:
  simple_readonly:
    model: cheap-fast-model
  code_edit:
    model: balanced-model
  security_sensitive:
    model: strongest-model
    require_confirmation: true
  failed_twice:
    escalate_to: stronger-model
```

Later routing can learn from historical traces.

### 7.11 CI Gates

AgentHarness should fail CI when reliability drops.

Example:

```yaml
fail_if:
  success_rate_drop_gt: 0.03
  cost_increase_gt: 0.20
  blocked_commands_gt: 0
  loop_failure_rate_gt: 0.05
  tool_error_rate_gt: 0.10
```

## 8. Rust Architecture

Use a Rust workspace.

```text
agentharness/
  Cargo.toml
  crates/
    agentharness-cli/
    agentharness-core/
    agentharness-trace/
    agentharness-policy/
    agentharness-eval/
    agentharness-router/
    agentharness-report/
    agentharness-adapters/
  examples/
    codegen.yaml
    risky-command.yaml
    prompt-regression.yaml
  runs/
```

### 8.1 Crate Responsibilities

```text
agentharness-cli
Command-line interface.

agentharness-core
Runs workflows, coordinates traces, policies, evaluators, and reports.

agentharness-trace
Event schema, span model, JSONL writer, run metadata.

agentharness-policy
Command safety, tool permissions, cost limits, loop cutoffs.

agentharness-eval
Rule-based evaluators, final result scoring, test result parsing.

agentharness-router
Model selection and task difficulty rules.

agentharness-report
Terminal and HTML reports.

agentharness-adapters
OpenAI, Anthropic, subprocess, Python, Node, and IDE integration helpers.
```

### 8.2 Suggested Rust Dependencies

Initial dependencies:

```toml
clap = "4"
serde = "1"
serde_json = "1"
serde_yaml = "0.9"
tokio = "1"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
uuid = "1"
chrono = "0.4"
regex = "1"
comfy-table = "7"
```

Later dependencies:

```toml
sqlx = "0.8"
reqwest = "0.12"
tera = "1"
schemars = "0.8"
wasmtime = "25"
```

## 9. Event Protocol

AgentHarness should use a simple JSONL event protocol so any language or agent can integrate.

Example:

```json
{"type":"run_started","task_id":"fix_tests","config":"gpt4_config_v2"}
{"type":"model_call","model":"claude","tokens_in":1800,"tokens_out":400,"cost_usd":0.018}
{"type":"tool_call","tool":"terminal","args":{"command":"cargo test"}}
{"type":"tool_call","tool":"terminal","args":{"command":"rm -rf target"}}
{"type":"final_answer","content":"Tests now pass."}
{"type":"run_finished","status":"success"}
```

This protocol enables integration with:

- OpenAI agents
- Claude workflows
- Cursor-like coding agents
- Python scripts
- TypeScript agents
- LangChain
- LangGraph
- local terminal workflows
- CI jobs
- internal company agents

## 10. Configuration Schema

Example test config:

```yaml
id: codegen_fix_tests
input: "Fix the failing tests in this Rust crate."

workflow:
  command: "python examples/run_agent.py"

limits:
  max_steps: 30
  max_cost_usd: 0.10
  max_duration_seconds: 300

allowed_tools:
  - terminal
  - file_read
  - file_write

blocked_commands:
  - "rm -rf /"
  - "git reset --hard"
  - "del /s C:\\"

required_checks:
  - "cargo test"

success_criteria:
  - tests_pass
  - no_blocked_commands
  - no_loop_detected
  - final_answer_mentions_evidence
```

## 11. CLI Design

### 11.1 Run

```bash
agentharness run <config-file> [--runs-dir runs] [--yes]
```

Runs the configured workflow and records traces.

Run v1 uses the minimal config shape:

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
- blocks `BLOCK` decisions without execution
- blocks `REQUIRE_CONFIRMATION` decisions unless `--yes` is passed
- writes `confirmation_response` for `REQUIRE_CONFIRMATION`
- executes `ALLOW`, `WARN`, and `REQUIRE_CONFIRMATION --yes` commands
- writes `terminal_command` with exit code, duration, and stdout/stderr excerpts
- writes `run_finished`
- updates `metadata.json`

Temporary v1 constraints:

- one command per config
- command strings execute through the platform shell (`cmd /C` on Windows, `sh -c` elsewhere)
- no interactive prompt yet; `--yes` is the temporary approval mechanism
- no model/tool/file/evaluation/suggestion events yet
- no working-directory config yet

### 11.2 Report

```bash
agentharness report <runs-dir-or-run-dir>
```

Shows run score, failures, warnings, costs, and suggestions.

Report v1 is the first trace reader. If the path contains `latest.txt`, it resolves that file to the latest run directory. Otherwise, it treats the path as a direct run directory.

Report v1 reads `metadata.json` and `events.jsonl`, then prints:

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
- no CI gate behavior
- no aggregation across multiple runs

### 11.3 Compare

```bash
agentharness compare runs/baseline runs/latest
```

Compares two runs or two run suites.

### 11.4 Policy Check

```bash
agentharness policy check "rm -rf /"
```

Classifies a command without running it.

### 11.5 Trace Demo

```bash
agentharness trace demo [--runs-dir runs] [--command "rm -rf /"]
```

Creates a demo trace without executing the command. This command is a temporary proof of wiring between `agentharness-cli`, `agentharness-policy`, and `agentharness-trace`.

It writes:

```text
run_started
policy_decision
```

It does not emit `terminal_command`, because it does not execute commands.

Temporary demo-only status mapping:

```text
ALLOW -> success
WARN -> success
REQUIRE_CONFIRMATION -> blocked
BLOCK -> blocked
```

For this demo, `success` means "classified as non-blocking", not "executed successfully". The real `agentharness run` command must revisit run status once commands actually execute.

### 11.6 CI

```bash
agentharness ci --config agentharness.yaml
```

Runs configured checks and exits non-zero on failure.

## 12. Implementation Roadmap

### Phase 1: Rust CLI Skeleton

Build:

- Rust workspace
- CLI commands
- YAML config parser
- run directory creation
- basic JSONL event writer

Commands:

```bash
agentharness run
agentharness report
agentharness policy check
```

### Phase 2: Trace Protocol

Build the ADR 002 trace foundation:

- `agentharness-trace` crate
- common trace event envelope
- v1 event types: `run_started`, `policy_decision`, `terminal_command`, `confirmation_response`, `run_finished`, `error`
- JSON serialization and deserialization
- synchronous JSONL append writer
- run directory creation
- `metadata.json`
- `latest.txt`
- CLI proof command: `agentharness trace demo [--runs-dir <path>] [--command "<cmd>"]`
- first real capture command: `agentharness run <config-file> [--runs-dir <path>] [--yes]`
- first trace reader command: `agentharness report <runs-dir-or-run-dir>`

Defer until later trace iterations:

- span IDs and parent-child span relationships
- model call events
- tool call events beyond terminal commands
- file change events
- evaluation and suggestion events
- duration and cost fields where needed by report/compare
- multi-command workflows
- interactive confirmation prompting
- working-directory config
- JSON report output
- scoring and CI gate behavior
- report aggregation across multiple runs

### Phase 3: Command Policy Engine

Build:

- regex-based dangerous command detection
- allow/warn/confirmation/block decisions
- Windows command rules
- Unix command rules
- Git command rules
- package manager command rules

This should be one of the first impressive features.

### Phase 4: Deterministic Evaluators

Build:

- command safety evaluator
- loop detector
- max cost evaluator
- max step evaluator
- required command evaluator
- final status evaluator
- tool contract evaluator

### Phase 5: Reports

Start with terminal reports.

Example:

```text
Run: latest
Status: FAILED

Passed: 6
Failed: 2
Warnings: 3
Cost: $0.037
Duration: 42s

Failures:
- Blocked command attempted: git reset --hard
- Required check missing: cargo test
```

Then add HTML reports.

### Phase 6: Compare

Build:

```bash
agentharness compare runs/baseline runs/latest
```

Compare:

- success rate
- average score
- cost
- latency
- failed checks
- blocked commands
- loop rate
- tool errors

### Phase 7: CI Mode

Build:

```bash
agentharness ci --config agentharness.yaml
```

The command should exit with a non-zero code when quality gates fail.

### Phase 8: Adapters

Build thin adapter packages:

```text
agentharness-python
agentharness-js
agentharness-github-action
agentharness-vscode
```

The Rust core remains the source of truth.

### Phase 9: LLM Judge

Add LLM-as-judge only after deterministic checks are useful.

Judge use cases:

- final answer quality
- hallucination suspicion
- context relevance
- prompt improvement suggestions
- task difficulty classification

### Phase 10: Model Router

Add:

```bash
agentharness route --task task.yaml
```

Then integrate routing into runs.

Start with rules. Later use historical run data.

## 13. MVP Scope

The first version should prove this:

> A developer can wrap an agent workflow, capture every step, block dangerous commands, evaluate the run, and compare two versions in CI.

MVP features:

- Rust CLI
- YAML test config
- JSONL traces
- terminal command policy checks
- loop detection
- cost and step limits
- basic terminal report
- baseline vs latest comparison
- CI exit codes

Explicitly postpone:

- full dashboard
- PostgreSQL
- cloud hosting
- complex LLM judge
- multi-user accounts
- large plugin marketplace

## 14. First Milestone

The first meaningful milestone:

```text
agentharness policy check "rm -rf /"
agentharness run examples/risky-command.yaml
agentharness report runs/latest
agentharness compare runs/baseline runs/latest
```

This milestone proves:

- the CLI works
- config parsing works
- traces are recorded
- dangerous commands are detected
- runs can be evaluated
- reports are useful
- comparison is possible

## 15. Long-Term Vision

AgentHarness can grow into a full agent reliability platform.

Future capabilities:

- IDE plugin for live command interception
- GitHub/GitLab CI integrations
- model provider adapters
- prompt version tracking
- policy packs for teams
- organization-wide safety rules
- historical performance database
- automatic model routing
- local sandbox execution
- trace replay
- human review queues
- LLM judge calibration
- team dashboards

## 16. Why Rust Is the Right Core

Rust fits the project because the core needs to be:

- fast
- safe
- reliable
- CI-friendly
- distributable as one binary
- good at process supervision
- good at structured event processing
- credible as developer infrastructure

The language reinforces the product promise: controlled, reliable agent execution.

## 17. Possible Names

Strong options:

- AgentHarness
- TraceGuard
- RunGuard
- AgentRail
- HarnessKit
- PromptRail

Recommended name:

```text
AgentHarness
```

It is clear, descriptive, and easy for developers to understand.

## 18. Final Product Thesis

Modern AI development is moving from single model capability to full system reliability.

AgentHarness should become the package developers add when they want their agentic workflows to be observable, testable, safer, cheaper, and more trustworthy.

The guiding sentence:

> AgentHarness is a Rust-based control and evaluation layer for agentic developer workflows. It traces prompts, context, tool calls, terminal commands, cost, and final outputs; blocks risky actions; detects loops and regressions; compares prompt/model/config versions; and gates agent behavior in CI.
