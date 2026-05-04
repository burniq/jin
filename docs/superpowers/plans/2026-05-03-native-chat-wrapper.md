# Native Chat Wrapper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first native-wrapper chat UX with durable chat sessions, tool descriptors, backend chat API, and a chat-first web interface.

**Architecture:** Add chat/session types to `jin-core`, expose chat endpoints from `jin-server`, and make `jin-web` render `/` as the primary chat workspace. The runtime boundary is explicit so the first Codex adapter can start as a persistent process bridge and later become a full PTY implementation without changing API or UI contracts.

**Tech Stack:** Rust 2021, Axum, Tokio, Serde, file-backed JSON state, server-rendered HTML.

---

### Task 1: Core Chat Domain

**Files:**
- Modify: `crates/jin-core/src/lib.rs`
- Create: `crates/jin-core/src/chat.rs`
- Modify: `crates/jin-core/src/store.rs`

- [ ] Add `chat.rs` with `ChatSession`, `ChatMessage`, `ToolDescriptor`, `ToolSettingDescriptor`, `ChatStatus`, and `ChatRole`.
- [ ] Export the module from `lib.rs`.
- [ ] Add `chats` and `chat_messages` vectors to persisted `JinState`.
- [ ] Write tests proving Codex has reasoning settings and shell does not.
- [ ] Run `cargo test -p jin-core chat`.

### Task 2: Orchestrator Chat Operations

**Files:**
- Modify: `crates/jin-core/src/orchestrator.rs`

- [ ] Add `CreateChatRequest` and `PostChatMessageRequest`.
- [ ] Add `list_tools`, `list_chats`, `get_chat`, `list_chat_messages`, `create_chat`, `append_chat_message`, and `set_chat_status`.
- [ ] Validate project and tool ids on chat creation.
- [ ] Validate blank messages on append.
- [ ] Write tests for create chat, unknown tool, unknown project, and message append.
- [ ] Run `cargo test -p jin-core orchestrator::tests::*chat*`.

### Task 3: Backend Chat Runtime and API

**Files:**
- Modify: `apps/jin-server/src/api.rs`

- [ ] Extend `AppState` with an in-memory chat runtime manager.
- [ ] Add routes `GET /tools`, `GET/POST /chats`, `GET /chats/{chat_id}`, `GET/POST /chats/{chat_id}/messages`, and `POST /chats/{chat_id}/stop`.
- [ ] For MVP runtime, persist user messages and append a tool output event from the runtime bridge. Keep the bridge interface named around persistent sessions.
- [ ] Add API tests for tools, chat creation, message posting, message listing, and stop.
- [ ] Run `cargo test -p jin-server chat`.

### Task 4: Chat-First Web UI

**Files:**
- Modify: `apps/jin-web/src/web.rs`

- [ ] Change `/` to render chat workspace instead of dashboard metrics.
- [ ] Add routes/forms for `POST /chats`, `GET /chats/{chat_id}`, `POST /chats/{chat_id}/messages`, and `POST /chats/{chat_id}/stop`.
- [ ] Render chat list, new chat form, selected chat header, capability-driven settings indicators, timeline, and bottom composer.
- [ ] Keep Projects, Approvals, Tasks, and Docs navigation.
- [ ] Add web tests for chat-first root and chat detail rendering.
- [ ] Run `cargo test -p jin-web chat`.

### Task 5: Docs and Verification

**Files:**
- Modify: `README.md`
- Modify: `apps/jin-web/src/web.rs`

- [ ] Update docs pages and README with chat endpoints and phone-first usage.
- [ ] Run `cargo fmt --all --check`.
- [ ] Run `cargo test --workspace`.
- [ ] Run `cargo clippy --workspace -- -D warnings`.
- [ ] Smoke test backend/frontend on alternate ports.
- [ ] Commit the implementation.
