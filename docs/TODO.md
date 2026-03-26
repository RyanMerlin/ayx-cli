# TODO

This is the working plan for evolving the AYX CLI into a command/tactic/skill registry that can be consumed by an agent-friendly layer.

## 1. Command registry
- define a compact machine-readable schema for the existing `clap` tree (name, purpose, args, output shape, safety level, mutating vs read-only).
- expose the schema through a new subcommand such as `ayx catalog list` / `ayx catalog describe <command>` so tooling can query it (JSON + CLI-friendly summary).
- annotate the schema with tactical hints (prerequisites, typical sequence, rollback, idempotency tags) during codegen or via manual metadata.
- ensure the schema is discoverable without dumping the entire manual (e.g., request only the branch the agent is working on).

## 2. Tactical registry
- create a compact format (YAML/JSON) for "tactics" that define small playbooks: trigger patterns, guardrails, execution hints, example commands, validation steps.
- add CLI helpers (`ayx tactics list`, `ayx tactics describe <tactic>`, `ayx tactics resolve --task "<text>"`) so the agent can lazily load the tactic that matches a high-level task.
- keep tactics scoped to command families and mark their safety (cross-check CLI safety level/tokens before invoking mutating commands).
- store audit/validation steps inside each tactic so workflows can verify success or roll back when needed.

## 3. Workflow/skill registry
- define higher-order workflows/skills that reference commands + tactics + validation, e.g., "governance-go-live", "backup-restore".
- capture workflow metadata (inputs, outputs, required tactics, typical CLI sequence) so the agent can plan end-to-end tasks.
- expose workflow introspection (`ayx workflows list`, `ayx workflows explain <name>`) to the orchestration layer.

## 4. Runtime resolver and injection
- build the resolver service that, given the current task/command, returns the minimal command/tactic/workflow context an agent needs.
- integrate execution history so the resolver can decide when to re-fire a tactic vs reuse prior context.
- emit structured evidence (plan/execute/verify/rollback steps) after each run so the agent can reason about outcomes without reloading every detail.

## 5. Documentation and examples
- update the README (and `docs/cli-spec.md`) with the recommended architecture summarised here.
- add a short walkthrough that shows how an agent would query the catalog/tactics/workflows before executing a workflow.

Next actions should focus on wiring a small prototype of step 1 before layering tactics/workflows on top.
