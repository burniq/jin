# jin Local MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `jin` from core contracts into a usable local MVP with HTTP control, durable file-backed state, approvals, local project execution, shell/Codex runners, and supervisor rollback commands.

**Architecture:** `jin-core` owns durable records, file store, orchestration, runner implementations, and Telegram normalization. `jin-server` exposes an HTTP API over the core orchestrator. `jin-supervisor` manages a version registry JSON file for status, promotion, and rollback without depending on the server process.

**Tech Stack:** Rust, `serde`, `serde_json`, `uuid`, `chrono`, `tokio`, `axum`, `clap`, `tempfile`, standard process execution.

---

## MVP Behavior

- Register local projects by name and root path.
- Submit tasks through HTTP using `shell` or `codex` runner.
- Shell tasks outside the allowlist become `waiting_approval`.
- Approving a task runs the pending shell command.
- Task records, approvals, projects, events, and version records survive process restarts through JSON files under `.jin/state`.
- `jin-server` exposes `/health`, `/projects`, `/tasks`, `/tasks/:id`, `/approvals/:id/approve`, `/approvals/:id/reject`, `/telegram/webhook`.
- `jin-supervisor` exposes `status`, `init-stable`, `set-candidate`, `promote`, and `rollback` over the same version registry format.

## Task 1: File Store And Orchestrator

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/jin-core/Cargo.toml`
- Create: `crates/jin-core/src/store.rs`
- Create: `crates/jin-core/src/orchestrator.rs`
- Modify: `crates/jin-core/src/lib.rs`

- [ ] Write failing tests for project registration, shell approval, approval execution, and state reload.
- [ ] Implement serializable records and JSON file store.
- [ ] Implement orchestrator methods used by tests.
- [ ] Run `cargo test --workspace`.

## Task 2: Process Runners

**Files:**
- Modify: `crates/jin-core/src/runner.rs`
- Modify: `crates/jin-core/src/orchestrator.rs`

- [ ] Write failing tests for shell command execution.
- [ ] Implement `ShellRunner` and `CodexRunner`.
- [ ] Run `cargo test --workspace`.

## Task 3: HTTP Server

**Files:**
- Modify: `apps/jin-server/Cargo.toml`
- Modify: `apps/jin-server/src/main.rs`
- Create: `apps/jin-server/src/api.rs`

- [ ] Write route-level tests for health, project registration, task submission, and approval.
- [ ] Implement axum routes and CLI flags.
- [ ] Run `cargo test --workspace`.

## Task 4: Supervisor CLI

**Files:**
- Modify: `apps/jin-supervisor/Cargo.toml`
- Modify: `apps/jin-supervisor/src/main.rs`

- [ ] Write tests for version registry commands through core behavior.
- [ ] Implement CLI over `VersionRegistry`.
- [ ] Run `cargo test --workspace`.

## Task 5: Docs And Verification

**Files:**
- Create: `README.md`

- [ ] Add local run instructions and HTTP examples.
- [ ] Run `cargo fmt --all`.
- [ ] Run `cargo test --workspace`.
- [ ] Commit the completed MVP.
