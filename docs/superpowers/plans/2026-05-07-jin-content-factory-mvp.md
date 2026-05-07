# Jin Content Factory MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a functional Content Factory MVP with project profiles, factory pipelines, Telegram-aware sync settings, API routes, and web-client factory screens.

**Architecture:** Add focused `factory` and `sync` core modules, persist factory/profile/sync metadata in `JinState`, expose factory/profile/settings APIs from `jin-server`, and render a separate `Factories` mode in the React client. Worker execution is represented by lifecycle/status APIs in this MVP; provider execution is added behind the same model later.

**Tech Stack:** Rust workspace (`jin-core`, `jin-server`), Axum JSON API, React/TypeScript Vite web client, file-backed JSON state.

---

### Task 1: Core Factory And Sync Model

**Files:**
- Create: `crates/jin-core/src/factory.rs`
- Create: `crates/jin-core/src/sync.rs`
- Modify: `crates/jin-core/src/lib.rs`
- Modify: `crates/jin-core/src/store.rs`
- Modify: `crates/jin-core/src/orchestrator.rs`

- [x] Write tests for creating a project content profile and factory pipeline with inherited sync defaults.
- [x] Verify the tests fail because the factory/sync modules and orchestrator methods do not exist.
- [x] Implement factory enums/records, sync target records, settings token redaction, and store persistence fields.
- [x] Implement orchestrator methods for profile upsert/list, factory create/list/get, event append, and pause/resume/stop.
- [x] Run `cargo test -p jin-core`.

### Task 2: Backend API

**Files:**
- Modify: `apps/jin-server/src/api.rs`

- [x] Write API tests for settings token redaction, content profile endpoints, factory create/list/get/events, and stop/resume.
- [x] Verify the tests fail on missing routes.
- [x] Add routes:
  - `GET /projects/{project}/content-profile`
  - `PUT /projects/{project}/content-profile`
  - `GET /factories`
  - `POST /factories`
  - `GET /factories/{factory_id}`
  - `GET /factories/{factory_id}/events`
  - `POST /factories/{factory_id}/pause`
  - `POST /factories/{factory_id}/resume`
  - `POST /factories/{factory_id}/stop`
- [x] Run `cargo test -p jin-server`.

### Task 3: Web Client API Types And Data Hook

**Files:**
- Modify: `apps/jin-web-client/src/api/types.ts`
- Modify: `apps/jin-web-client/src/api/client.ts`
- Modify: `apps/jin-web-client/src/hooks/useJinData.ts`

- [x] Write client tests for factory API methods and settings payload shape.
- [x] Verify the tests fail.
- [x] Add TypeScript types and client methods.
- [x] Load factories in `useJinData`.
- [x] Run `npm test -- client`.

### Task 4: Web Factories UI

**Files:**
- Create: `apps/jin-web-client/src/components/FactoriesView.tsx`
- Modify: `apps/jin-web-client/src/App.tsx`
- Modify: `apps/jin-web-client/src/components/AppShell.tsx`
- Modify: `apps/jin-web-client/src/components/SettingsView.tsx`
- Modify: `apps/jin-web-client/src/styles.css`

- [x] Write component tests for navigation, factory creation, factory detail, and sync selection.
- [x] Verify the tests fail.
- [x] Add `Factories` navigation, list/detail/create UI, and settings controls for Telegram/default sync.
- [x] Run `npm test`.

### Task 5: Verification And Docs

**Files:**
- Modify: `README.md`

- [x] Document factory MVP startup and API surface.
- [x] Run `cargo test --workspace`.
- [x] Run `npm test` in `apps/jin-web-client`.
- [x] Run `npm run build` in `apps/jin-web-client`.
- [ ] Commit the implementation.
