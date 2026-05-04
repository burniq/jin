# jin Web Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a separate `jin-web` frontend process with a simple web UI and built-in documentation pages for every MVP function.

**Architecture:** Keep `jin-server` as a JSON API process and add `jin-web` as a separate Axum server on its own port. `jin-web` calls the backend server-side using `JIN_API_BASE` and `JIN_API_TOKEN`, then renders HTML pages and form flows for projects, tasks, approvals, and docs.

**Tech Stack:** Rust, Axum, Reqwest, server-rendered HTML, existing `jin-core` serializable records.

---

## Routes

- `/`: dashboard with backend health, project/task/approval counts, and navigation.
- `/projects`: list projects and register a local project.
- `/tasks`: list tasks and create shell/codex tasks.
- `/tasks/{task_id}`: show task details and output.
- `/approvals`: list approvals and approve/reject pending items.
- `/docs`: docs index.
- `/docs/http-api`: backend JSON API.
- `/docs/telegram`: Telegram webhook commands.
- `/docs/runners`: shell and codex runners.
- `/docs/supervisor`: stable/candidate/promotion/rollback.
- `/docs/state`: file-backed state.
- `/docs/security`: token auth and approvals.

## Tasks

- [ ] Add `apps/jin-web` to the Rust workspace.
- [ ] Write route/render tests for dashboard and docs pages.
- [ ] Implement backend client with bearer token kept server-side.
- [ ] Implement HTML rendering helpers and navigation.
- [ ] Implement projects/tasks/approvals pages and form handlers.
- [ ] Update README with two-process run instructions.
- [ ] Run `cargo fmt --all`, `cargo test --workspace`, and `cargo clippy --workspace -- -D warnings`.
