# Native Chat Wrapper Design

## Goal

`jin` should feel like a phone-friendly web wrapper for native development agent tools, starting with Codex CLI. The main web flow is a chat workspace, not an admin dashboard.

## Product Direction

`jin` is a central multitool for operating local development agents from a phone. It does not hide all tools behind a generic agent abstraction. Instead, it preserves each tool's native concepts and exposes them through one consistent web shell.

The selected direction is **Native Wrapper**:

- `jin` owns projects, chats, durable timeline, approvals, and web/mobile UX.
- Each tool adapter declares its own capabilities and settings.
- The UI renders only settings that the selected tool supports.
- Codex integration is through a persistent Codex CLI session, not separate stateless `codex exec` jobs.

## Core Scenarios

1. Create a chat from the phone by selecting a project, tool, and supported tool settings.
2. Send messages into a live Codex CLI session.
3. Read the session timeline after reconnecting from the phone.
4. See native-adjacent indicators: current tool, model/reasoning controls where supported, context meter where supported, working directory, and session status.
5. Stop or continue a running session.
6. Keep admin pages for projects, approvals, tasks, and docs as secondary surfaces.

## Tool Capabilities

Tools expose a descriptor instead of relying on global UI assumptions.

For the MVP:

- `codex` supports model/reasoning settings, context indicators, approval mode, and persistent sessions.
- `shell` supports explicit command execution and does not expose intelligence controls.
- Future tools such as Claude can add their own settings without changing the chat shell.

Unavailable settings must not be shown. If a user switches tools later, unsupported settings are hidden or reset by the server.

## Session Model

`ChatSession` is separate from the existing task model.

It includes:

- stable id;
- title;
- project;
- tool id;
- status;
- tool settings;
- context summary;
- created/updated timestamps.

`ChatMessage` is the durable timeline event:

- user message;
- assistant output;
- tool/process output;
- system/status message;
- error message;
- future approval request.

## Codex Runtime

The desired runtime model is a persistent process owned by the backend. The frontend can disconnect and reconnect without killing the session. A message submitted from the web UI is written to the process stdin. Output is collected into the session timeline.

The first implementation may use a conservative line-based process bridge while keeping the runtime boundary named and shaped as a Codex session adapter. That leaves room to replace the internals with a real PTY implementation without changing web/API contracts.

## Web UX

The root page `/` becomes the chat workspace.

The page includes:

- chat list;
- new chat form;
- chat header with project, tool, and status;
- compact context/settings indicators;
- collapsible settings area;
- timeline;
- bottom composer;
- stop/continue controls where available.

Admin pages remain:

- `/projects`;
- `/approvals`;
- `/tasks` as legacy/debug;
- `/docs`.

## API

The backend exposes JSON endpoints for chat UX:

- `GET /tools`;
- `GET /chats`;
- `POST /chats`;
- `GET /chats/{chat_id}`;
- `GET /chats/{chat_id}/messages`;
- `POST /chats/{chat_id}/messages`;
- `POST /chats/{chat_id}/stop`.

These endpoints use the same bearer token rules as existing backend endpoints.

## Error Handling

Invalid project/tool/settings produce `400 Bad Request`. Unknown chats produce `404 Not Found`. Runtime failures become durable error messages in the timeline and set the chat status to `Error`.

## Testing

Tests should cover:

- tool descriptors expose Codex-specific settings and hide intelligence controls for shell;
- chat creation validates project and tool;
- posting a message persists the user message and creates output from a fake persistent runtime;
- API endpoints return expected status and JSON;
- web root renders chat-first UI and does not present the old dashboard as the primary experience.
