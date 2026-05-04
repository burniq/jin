# Jin Web Client Redesign

## Goal

Replace the current server-rendered chat page with a normal web client that feels close to Codex/ChatGPT/Claude: focused chat creation, predictable navigation, explicit tool settings, and readable live agent progress.

The selected direction is **desktop/tablet shell + mobile chat-first collapse**:

- Desktop/tablet: persistent left sidebar with chats and global navigation, full chat surface on the right.
- Mobile: chat-first screen with chats/settings behind panels, so the primary screen is always the conversation.

## Problems To Fix

The current `jin-web` page is overloaded because it combines several responsibilities in one server-rendered document:

- chat list,
- new chat creation,
- selected chat timeline,
- mutable settings,
- progress polling,
- docs/navigation controls.

This produces unstable visual hierarchy and makes the phone flow feel unnatural. The next implementation should stop adding patches to the single HTML renderer and introduce a real frontend boundary.

## Product Flow

### New Chat

The root route opens a focused new-chat composer.

Required controls:

- first prompt textarea,
- project selector,
- tool selector,
- model selector when the selected tool exposes models,
- reasoning/intelligence selector only when the selected tool exposes reasoning,
- optional title hidden behind a secondary details/settings affordance.

Submitting the composer creates the chat and immediately sends the first prompt when it is non-empty.

### Existing Chat

An existing chat view contains:

- header with chat title, project, tool, model, reasoning, context meter/status,
- timeline with user messages and agent/tool output,
- composer pinned to the bottom,
- stop button while a session is running,
- settings drawer for mutable model/reasoning/tool options.

Changing settings should call the existing settings API and update visible chips without requiring a full page reload.

### Progress

Agent progress must be readable and stable:

- show a small "agent working" indicator when status is `Running`,
- append streamed/progress output into the timeline without layout churn,
- preserve stopped/error/waiting statuses,
- avoid duplicate output when polling returns the same timeline.

The first MVP can continue using polling against `/chats/{chat_id}/progress`; the frontend API layer should isolate that detail so a later SSE/WebSocket transport can replace it.

## Frontend Architecture

Create a new frontend app under `apps/jin-web-client`.

Recommended stack:

- Vite,
- TypeScript,
- React,
- CSS modules or plain colocated CSS,
- no heavy design framework in the MVP.

The existing Rust `apps/jin-web` can remain temporarily as the legacy server-rendered client until the new client reaches parity. The backend `apps/jin-server` remains the source of truth for data and native tool sessions.

Frontend modules:

- `api/`: typed client for backend endpoints.
- `routes/`: route-level screens for new chat, chat detail, projects, docs/approvals later.
- `components/chat/`: sidebar, timeline, composer, settings drawer, progress indicator.
- `components/layout/`: desktop shell and mobile panels.
- `state/`: local hooks for chats, tools, selected chat, progress polling.

## API Surface

Use the existing backend endpoints first:

- `GET /tools`
- `GET /projects`
- `GET /chats`
- `POST /chats`
- `GET /chats/{chat_id}`
- `GET/POST /chats/{chat_id}/messages`
- `GET /chats/{chat_id}/progress`
- `POST /chats/{chat_id}/settings`
- `POST /chats/{chat_id}/stop`

The web client should read the backend base URL from environment configuration. During development, Vite should proxy `/api/*` to `jin-server`, or the client should receive a configured API base URL.

## Layout Behavior

### Desktop And Tablet

Use a two-column app shell:

- left column: `jin`, New Chat button, chat list, links to Projects/Approvals/Docs,
- right column: selected chat or focused new-chat screen.

The new-chat form must not live inside the chat list.

### Mobile

Use one primary column:

- default screen is either New Chat or the selected conversation,
- chat list opens as a drawer/sheet,
- settings open as a drawer/sheet,
- composer stays reachable at the bottom.

## Error Handling

The frontend should show clear inline errors for:

- backend unavailable,
- project list unavailable,
- tool catalog unavailable,
- failed chat creation,
- failed message send,
- failed settings update.

Errors should not replace the whole app shell unless the app cannot start at all.

## Testing

Unit/component tests should cover:

- new chat composer renders tool-specific controls,
- shell tools do not show Codex-only settings,
- root route does not auto-open the latest chat,
- creating a chat with an initial prompt sends both create and first message calls,
- chat progress polling renders new messages and status,
- stopped/error statuses are not displayed as running.

End-to-end or smoke verification should cover:

- backend and frontend running on separate ports,
- creating a chat from the new frontend,
- opening an existing chat,
- updating model/reasoning,
- observing progress output appear without a manual refresh.

## Migration Plan

1. Scaffold `apps/jin-web-client`.
2. Build typed API client and app shell.
3. Implement New Chat and Chat Detail flows.
4. Add progress polling and settings drawer.
5. Add docs links/pages needed for parity.
6. Keep `apps/jin-web` available until the new client is usable, then decide whether to remove it or keep it as a lightweight fallback.

## Non-Goals For This Step

- Replacing polling with WebSocket/SSE.
- Rebuilding every docs/admin page before chat parity.
- Adding auth UI; existing token/backend config remains server/developer configured for now.
- Building a full design system.
