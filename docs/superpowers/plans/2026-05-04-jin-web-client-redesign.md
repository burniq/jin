# Jin Web Client Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a separate React/Vite web client for Jin with a Codex-like desktop shell and a chat-first mobile layout.

**Architecture:** `apps/jin-web-client` becomes the primary frontend and talks directly to `jin-server` JSON endpoints. The legacy Rust `apps/jin-web` remains as a fallback during migration. Frontend code is split into typed API access, app state hooks, layout components, chat components, and route-level screens.

**Tech Stack:** Vite, React, TypeScript, Vitest, Testing Library, plain CSS.

---

## File Structure

- Create `apps/jin-web-client/package.json`: scripts and frontend dependencies.
- Create `apps/jin-web-client/index.html`: Vite entry document.
- Create `apps/jin-web-client/vite.config.ts`: React plugin, Vitest jsdom, `/api` proxy to `127.0.0.1:8787`.
- Create `apps/jin-web-client/tsconfig.json`: TypeScript app config.
- Create `apps/jin-web-client/src/main.tsx`: React bootstrap.
- Create `apps/jin-web-client/src/api/types.ts`: backend DTO types.
- Create `apps/jin-web-client/src/api/client.ts`: typed backend client.
- Create `apps/jin-web-client/src/hooks/useJinData.ts`: load tools/projects/chats and expose refresh helpers.
- Create `apps/jin-web-client/src/hooks/useChatProgress.ts`: polling for `/chats/{id}/progress`.
- Create `apps/jin-web-client/src/components/AppShell.tsx`: desktop shell and mobile drawer behavior.
- Create `apps/jin-web-client/src/components/NewChat.tsx`: focused new-chat composer.
- Create `apps/jin-web-client/src/components/ChatView.tsx`: chat header, timeline, composer, progress, settings drawer.
- Create `apps/jin-web-client/src/components/DocsView.tsx`: simple docs pages for visible MVP functions.
- Create `apps/jin-web-client/src/App.tsx`: route selection and app composition.
- Create `apps/jin-web-client/src/styles.css`: responsive layout and component styling.
- Create focused tests next to source files with `.test.tsx` suffix.

## Task 1: Scaffold Frontend Package

**Files:**
- Create: `apps/jin-web-client/package.json`
- Create: `apps/jin-web-client/index.html`
- Create: `apps/jin-web-client/vite.config.ts`
- Create: `apps/jin-web-client/tsconfig.json`
- Modify: `.gitignore`

- [ ] **Step 1: Write package and toolchain files**

Create `package.json` with:

```json
{
  "name": "jin-web-client",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite --host 127.0.0.1 --port 8790",
    "build": "tsc --noEmit && vite build",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "@vitejs/plugin-react": "^latest",
    "vite": "^latest",
    "typescript": "^latest",
    "react": "^latest",
    "react-dom": "^latest",
    "lucide-react": "^latest"
  },
  "devDependencies": {
    "@testing-library/jest-dom": "^latest",
    "@testing-library/react": "^latest",
    "@testing-library/user-event": "^latest",
    "jsdom": "^latest",
    "vitest": "^latest"
  }
}
```

Create `vite.config.ts` with React plugin, jsdom tests, and `/api` proxy. Create `index.html` with a `#root` div. Create `tsconfig.json` with `jsx: react-jsx`, `strict: true`, and DOM libs. Add `node_modules/` and `dist/` to `.gitignore`.

- [ ] **Step 2: Install dependencies**

Run:

```bash
npm install
```

Expected: `package-lock.json` is created and dependencies install without audit blocking the task.

- [ ] **Step 3: Verify empty package scripts are wired**

Run:

```bash
npm test
```

Expected before app files exist: Vitest exits with no tests or an import error that names missing `src` files. Continue to Task 2.

## Task 2: Typed API Client

**Files:**
- Create: `apps/jin-web-client/src/api/types.ts`
- Create: `apps/jin-web-client/src/api/client.ts`
- Create: `apps/jin-web-client/src/api/client.test.ts`

- [ ] **Step 1: Write failing API client tests**

`client.test.ts` should mock `global.fetch` and assert:

```ts
await client.createChat({
  project: "jin",
  tool: "codex",
  title: null,
  settings: { model: "gpt-5.5", reasoning: "high" }
});
```

uses `POST /api/chats`, JSON body, and returns the decoded chat. Add a second test that a `400` response with `{ "error": "bad project" }` rejects with `bad project`.

- [ ] **Step 2: Run API tests and verify RED**

Run:

```bash
npm test -- src/api/client.test.ts
```

Expected: FAIL because `createJinApiClient` does not exist.

- [ ] **Step 3: Implement API types and client**

Define DTOs for `ToolDescriptor`, `ProjectRecord`, `ChatSession`, `ChatMessage`, `ChatProgressResponse`, `CreateChatPayload`, `SendChatMessagePayload`, and `UpdateChatSettingsPayload`.

Implement:

```ts
export function createJinApiClient(baseUrl = "/api"): JinApiClient
```

with methods for tools, projects, chats, messages, progress, settings, and stop. Every request should parse backend `{ error }` responses and throw `Error(error)`.

- [ ] **Step 4: Run API tests and verify GREEN**

Run:

```bash
npm test -- src/api/client.test.ts
```

Expected: PASS.

## Task 3: App Data Hooks

**Files:**
- Create: `apps/jin-web-client/src/hooks/useJinData.ts`
- Create: `apps/jin-web-client/src/hooks/useChatProgress.ts`
- Create: `apps/jin-web-client/src/hooks/useChatProgress.test.tsx`

- [ ] **Step 1: Write failing progress hook test**

Render a test component using `useChatProgress({ chatId: "chat-1", api, enabled: true })`. Mock `api.getChatProgress` to return a `Running` chat and one tool message. Assert the component renders the message after timers advance.

- [ ] **Step 2: Run hook test and verify RED**

Run:

```bash
npm test -- src/hooks/useChatProgress.test.tsx
```

Expected: FAIL because `useChatProgress` does not exist.

- [ ] **Step 3: Implement hooks**

`useJinData` loads tools, projects, and chats on mount, exposes `refreshChats`, and keeps inline `loading`/`error` state.

`useChatProgress` polls every 1200ms when enabled and calls `onProgress(progress)` with the full backend response. It must clear the interval on unmount and avoid polling with no `chatId`.

- [ ] **Step 4: Run hook test and verify GREEN**

Run:

```bash
npm test -- src/hooks/useChatProgress.test.tsx
```

Expected: PASS.

## Task 4: New Chat Composer

**Files:**
- Create: `apps/jin-web-client/src/components/NewChat.tsx`
- Create: `apps/jin-web-client/src/components/NewChat.test.tsx`

- [ ] **Step 1: Write failing composer tests**

Test that Codex tools render `Project`, `Tool`, `Model`, and `Reasoning`, while shell tools do not render `Model` or `Reasoning`. Test submit with first prompt calls `api.createChat` and then `api.sendMessage`.

- [ ] **Step 2: Run composer tests and verify RED**

Run:

```bash
npm test -- src/components/NewChat.test.tsx
```

Expected: FAIL because `NewChat` does not exist.

- [ ] **Step 3: Implement composer**

Build a focused composer with textarea first, compact toolbar below, and secondary optional title. Tool-specific settings should be derived from `ToolDescriptor.settings`, not hardcoded only for Codex.

- [ ] **Step 4: Run composer tests and verify GREEN**

Run:

```bash
npm test -- src/components/NewChat.test.tsx
```

Expected: PASS.

## Task 5: Chat View

**Files:**
- Create: `apps/jin-web-client/src/components/ChatView.tsx`
- Create: `apps/jin-web-client/src/components/ChatView.test.tsx`

- [ ] **Step 1: Write failing chat view tests**

Test that `Running` shows an agent progress indicator, `Stopped` does not show "working", settings submit calls `api.updateChatSettings`, and sending a message calls `api.sendMessage`.

- [ ] **Step 2: Run chat view tests and verify RED**

Run:

```bash
npm test -- src/components/ChatView.test.tsx
```

Expected: FAIL because `ChatView` does not exist.

- [ ] **Step 3: Implement chat view**

Render header chips, context label, timeline, progress indicator, bottom composer, stop button, and a settings drawer/details panel. Keep layout stable: messages scroll inside the timeline; composer stays pinned.

- [ ] **Step 4: Run chat view tests and verify GREEN**

Run:

```bash
npm test -- src/components/ChatView.test.tsx
```

Expected: PASS.

## Task 6: App Shell And Routing

**Files:**
- Create: `apps/jin-web-client/src/components/AppShell.tsx`
- Create: `apps/jin-web-client/src/components/DocsView.tsx`
- Create: `apps/jin-web-client/src/App.tsx`
- Create: `apps/jin-web-client/src/App.test.tsx`
- Create: `apps/jin-web-client/src/main.tsx`
- Create: `apps/jin-web-client/src/styles.css`

- [ ] **Step 1: Write failing app shell tests**

Test that `/` renders "Start a new chat" and does not auto-open latest chat. Test that `/chats/chat-1` renders the selected chat. Test that mobile drawer controls are present with accessible labels.

- [ ] **Step 2: Run app tests and verify RED**

Run:

```bash
npm test -- src/App.test.tsx
```

Expected: FAIL because `App` does not exist.

- [ ] **Step 3: Implement shell and routes**

Use browser location path for minimal routing:

- `/` -> New Chat,
- `/chats/:id` -> Chat View,
- `/docs` -> Docs View.

Desktop shell shows sidebar. Mobile shell has buttons for chats/settings panels. Chat list links update `window.history.pushState`.

- [ ] **Step 4: Run app tests and verify GREEN**

Run:

```bash
npm test -- src/App.test.tsx
```

Expected: PASS.

## Task 7: Full Verification And Dev Server

**Files:**
- Modify only files needed by failing checks.

- [ ] **Step 1: Run frontend checks**

Run:

```bash
npm test
npm run build
```

Expected: PASS.

- [ ] **Step 2: Run backend/workspace checks**

Run from repo root:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS.

- [ ] **Step 3: Start frontend dev server**

Run:

```bash
npm run dev
```

Expected: Vite serves the new frontend at `http://127.0.0.1:8790/` and proxies backend API calls to `http://127.0.0.1:8787/`.

- [ ] **Step 4: Commit**

Commit the complete frontend with:

```bash
git add .gitignore apps/jin-web-client
git commit -m "Build React web client"
```
