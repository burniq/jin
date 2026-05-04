# jin Initial Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the initial `jin/` Rust workspace with core domain contracts for commands, tasks, policy approvals, local repositories, runners, and self-maintenance rollback.

**Architecture:** Keep the first implementation focused on testable core behavior. The `jin-core` crate owns protocol-neutral domain types and contracts; `jin-server` and `jin-supervisor` are small binaries that compile against the workspace and will be expanded in later increments.

**Tech Stack:** Rust workspace, standard library only for the first core slice, `cargo test` verification.

---

## File Structure

- `jin/Cargo.toml`: workspace manifest.
- `jin/rust-toolchain.toml`: pinned stable toolchain.
- `jin/.gitignore`: Rust build artifacts.
- `jin/crates/jin-core/Cargo.toml`: core library crate manifest.
- `jin/crates/jin-core/src/lib.rs`: public module exports.
- `jin/crates/jin-core/src/command.rs`: input-source and command envelope types.
- `jin/crates/jin-core/src/task.rs`: durable task and approval lifecycle state.
- `jin/crates/jin-core/src/policy.rs`: approval policy rules for guarded actions.
- `jin/crates/jin-core/src/repository.rs`: local filesystem project registry.
- `jin/crates/jin-core/src/runner.rs`: runner adapter contract and synchronous test runner.
- `jin/crates/jin-core/src/version.rs`: version registry, candidate promotion, and rollback model.
- `jin/apps/jin-server/Cargo.toml`: server binary manifest.
- `jin/apps/jin-server/src/main.rs`: placeholder server entrypoint that links core.
- `jin/apps/jin-supervisor/Cargo.toml`: supervisor binary manifest.
- `jin/apps/jin-supervisor/src/main.rs`: placeholder supervisor entrypoint that links core.

## Task 1: Bootstrap Rust Workspace

**Files:**
- Create: `jin/Cargo.toml`
- Create: `jin/rust-toolchain.toml`
- Create: `jin/.gitignore`
- Create: `jin/crates/jin-core/Cargo.toml`
- Create: `jin/crates/jin-core/src/lib.rs`
- Create: `jin/apps/jin-server/Cargo.toml`
- Create: `jin/apps/jin-server/src/main.rs`
- Create: `jin/apps/jin-supervisor/Cargo.toml`
- Create: `jin/apps/jin-supervisor/src/main.rs`

- [ ] **Step 1: Create manifests and minimal binaries**

Create a Rust workspace with members `crates/jin-core`, `apps/jin-server`, and `apps/jin-supervisor`. The binaries should print their component names and compile without external dependencies.

- [ ] **Step 2: Run workspace tests**

Run: `cargo test --workspace`

Expected: command succeeds with no tests yet.

## Task 2: Add Command, Task, Policy, Repository, Runner, And Version Tests

**Files:**
- Modify: `jin/crates/jin-core/src/lib.rs`
- Create: `jin/crates/jin-core/src/command.rs`
- Create: `jin/crates/jin-core/src/task.rs`
- Create: `jin/crates/jin-core/src/policy.rs`
- Create: `jin/crates/jin-core/src/repository.rs`
- Create: `jin/crates/jin-core/src/runner.rs`
- Create: `jin/crates/jin-core/src/version.rs`

- [ ] **Step 1: Write failing tests first**

Add tests that require:

- `CommandEnvelope::new` rejects blank command text.
- a task can enter `waiting_approval` and then become `running` after approval.
- policy requires approval for non-allowlisted shell commands and `jin` promotion.
- local repository registry rejects workspaces outside registered roots.
- fake runner records output and returns a structured result.
- version registry promotes a healthy candidate and rolls back to the previous stable artifact.

- [ ] **Step 2: Verify tests fail**

Run: `cargo test --workspace`

Expected: failure from missing core types and methods.

## Task 3: Implement Core Domain Behavior

**Files:**
- Modify: `jin/crates/jin-core/src/command.rs`
- Modify: `jin/crates/jin-core/src/task.rs`
- Modify: `jin/crates/jin-core/src/policy.rs`
- Modify: `jin/crates/jin-core/src/repository.rs`
- Modify: `jin/crates/jin-core/src/runner.rs`
- Modify: `jin/crates/jin-core/src/version.rs`

- [ ] **Step 1: Implement minimal production code**

Implement only the behavior required by the tests from Task 2.

- [ ] **Step 2: Verify tests pass**

Run: `cargo test --workspace`

Expected: all tests pass.

## Task 4: Run Final Checks

**Files:**
- No file changes expected.

- [ ] **Step 1: Format**

Run: `cargo fmt --all`

Expected: no formatting errors.

- [ ] **Step 2: Test**

Run: `cargo test --workspace`

Expected: all tests pass.

- [ ] **Step 3: Inspect status**

Run: `git status --short`

Expected: only `jin/` and this plan file should be new or modified by this implementation.
