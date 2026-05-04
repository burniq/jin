# jin Agent Master Design

## Goal

Build `jin`, a phone-controlled master agent for software development work.

`jin` receives commands from mobile-friendly interfaces, turns them into durable tasks, and executes those tasks through pluggable development runners such as Codex and controlled shell commands. The design must keep integrations replaceable, preserve a clear audit trail, and make self-improvement safe enough that `jin` can help develop itself without becoming hard to recover.

## Product Shape

`jin` is a personal development control plane, not a general chatbot.

The first useful version focuses on:

- Telegram control from a phone
- exposed HTTP API for scripts and future clients
- Codex runner integration
- controlled shell runner integration
- local filesystem repository access
- explicit approvals for risky actions
- safe self-maintenance and rollback

The architecture intentionally keeps extension points for more input channels, more runners, and more repository providers, but those are not part of the initial implementation.

## Users And Scope

The initial user model is single-owner, personal use.

In scope:

- issuing development commands from Telegram
- issuing commands through an authenticated HTTP API
- selecting a registered local project
- asking Codex to inspect, modify, test, and summarize work
- running controlled shell commands for repository inspection, builds, and tests
- asking for approval before risky or irreversible actions
- improving `jin` itself through the same task system
- rolling back `jin` to the last known-good stable version

Out of scope for the first version:

- multi-user teams
- billing
- public marketplace integrations
- full web dashboard
- mobile native app
- Slack, WhatsApp, Discord
- GitHub/GitLab repository providers
- Claude Code and remote workers
- autonomous deployment without human approval

## High-Level Architecture

`jin` has these major components:

### 1. Input Adapters

Input adapters receive external user intent and convert it into a common internal command format.

Initial adapters:

- Telegram bot adapter
- HTTP API adapter

Deferred adapters:

- Slack
- WhatsApp
- Discord
- web dashboard
- native mobile app
- webhook-specific adapters

Each input adapter emits a `CommandEnvelope` with:

- source adapter
- authenticated actor
- conversation or request ID
- raw command text or structured payload
- optional target project
- reply channel metadata
- timestamp

Adapters do not run tasks directly. They only authenticate, normalize, and hand commands to the command gateway.

### 2. Command Gateway

The command gateway validates and routes incoming `CommandEnvelope` instances.

Responsibilities:

- authenticate requests
- reject unknown actors
- normalize command syntax
- apply basic rate limits
- resolve target project when possible
- create or resume conversation context
- hand accepted commands to the orchestrator

The gateway is the boundary between external protocols and the internal task model.

### 3. Orchestrator

The orchestrator is the decision and coordination center.

Responsibilities:

- create durable tasks
- select an appropriate runner
- track task lifecycle state
- stream progress updates
- request approvals
- cancel tasks
- resume tasks after restart
- summarize results back to the originating input adapter

Task states:

- `queued`
- `running`
- `waiting_approval`
- `cancelled`
- `failed`
- `completed`

The orchestrator must treat every runner as replaceable through a runner adapter contract.

### 4. Runner Adapters

Runner adapters execute work for a task.

Initial runners:

- `codex` runner for development tasks
- `shell` runner for controlled command execution

Deferred runners:

- Claude Code
- GitHub Actions
- remote worker processes
- model-provider-specific tools
- project-specific scripts

The runner adapter contract should support:

- task start
- streamed output events
- structured final result
- cancellation
- working directory selection
- environment profile selection
- declared required capabilities
- declared approval requests

Runners cannot bypass the orchestrator's policy and approval model.

### 5. Repository Provider

The repository provider gives `jin` controlled access to project workspaces.

Initial provider:

- local filesystem provider

The local provider manages registered project roots and validates that task working directories stay inside approved paths.

Deferred providers:

- GitHub
- GitLab
- Bitbucket
- remote clone workers
- cloud development environments

The provider contract should eventually cover:

- list registered projects
- inspect repository metadata
- create task workspace
- expose git status
- create branch or worktree
- produce diff summary
- clean up task workspace

For the first version, local filesystem access is enough.

### 6. Policy Engine And Approvals

The policy engine decides which actions can proceed automatically and which require explicit approval.

Initial approval gates:

- shell command execution outside a small allowlist
- file writes in sensitive paths
- git push
- publishing pull requests
- dependency installation
- network access from runners when not already allowed by task profile
- changes to `jin` itself
- promotion of a new `jin` version
- rollback confirmation if the command could discard an in-progress candidate

Approvals are first-class task state, not chat-only messages. A task waiting for approval must survive process restarts.

Approval prompts must include:

- requested action
- task ID
- project
- command or operation
- risk summary
- proposed actor or runner
- approve and reject actions

### 7. Notification Bus

The notification bus sends task updates back to users through the originating adapter or another configured channel.

Events include:

- task accepted
- task started
- progress summary
- approval requested
- approval accepted or rejected
- task failed
- task completed
- rollback started
- rollback completed

Notifications should be concise by default and link to fuller logs or summaries where available.

### 8. Durable Store

`jin` needs durable state for tasks, approvals, project registry, runner events, audit records, and version registry.

The recommended first database is PostgreSQL because it supports durable jobs, transactional approval state, and future multi-process workers without redesign.

SQLite is acceptable only for a very early prototype, not for the planned architecture.

## Data Flow

### Command Flow

1. User sends a Telegram message or HTTP API request.
2. Input adapter authenticates the source and creates a `CommandEnvelope`.
3. Command gateway validates and normalizes the command.
4. Orchestrator creates or updates a durable task.
5. Policy engine checks whether the task can start.
6. Orchestrator selects a runner adapter.
7. Runner executes in the selected local project workspace.
8. Runner emits progress and final result events.
9. Orchestrator records events and summarizes the result.
10. Notification bus replies through Telegram or API response/event stream.

### Approval Flow

1. Runner or orchestrator requests a guarded operation.
2. Policy engine creates an approval request.
3. Task state becomes `waiting_approval`.
4. Notification bus sends an approval prompt.
5. User approves or rejects through Telegram or HTTP API.
6. Orchestrator resumes or cancels the guarded operation.
7. Audit log records the decision.

## Self-Maintenance And Rollback

Safe self-maintenance is a foundational requirement.

`jin` may help develop `jin`, but the running `jin` process must not be the only recovery mechanism.

The self-maintenance model has four stages:

1. Propose: `jin` creates a task against its own repository.
2. Verify: the candidate version must pass build, tests, smoke checks, migration checks, and policy checks.
3. Approve: promotion of changes to `jin` requires explicit human approval.
4. Promote: a supervisor moves the stable version pointer after successful health checks.

Rollback requirements:

- user can issue a simple rollback command, such as `/rollback`
- rollback switches to the last known-good stable version
- rollback is handled by a small external supervisor
- rollback must not depend on the current `jin` process being healthy
- the version registry tracks stable, candidate, previous stable, artifact path, source commit, migration status, and health result

The stable version should be recorded as both:

- source commit or git tag
- built release artifact

Rollback should execute against the artifact because that is faster and more reliable during recovery.

Key rule:

```text
jin must not be its own only recovery mechanism.
```

## Deployment Shape

The architecture supports both local and server deployment, but the first implementation should start as a local daemon with a clean path to server deployment.

Recommended initial shape:

- `jin-server`: long-running local daemon exposing HTTP API and Telegram polling or webhook handling
- `jin-worker`: task runner process, initially colocated with the server
- `jin-supervisor`: small process or service wrapper responsible for start, stop, promote, rollback, and health checks
- PostgreSQL for durable state
- registered local project roots under explicit configuration

This keeps the first version practical while preserving the boundaries needed for future VPS or home-server deployment.

## Security Boundaries

Core boundaries:

- input adapters do not execute tasks
- runners do not approve their own risky actions
- repository providers validate workspace boundaries
- policy decisions are centralized
- approvals are durable and auditable
- self-updates require stricter gates than normal project tasks
- rollback is owned by the supervisor, not the live `jin` process

Secrets should be injected through environment profiles or a secret provider abstraction. Runners should receive only the secrets needed for the selected task profile.

## Extension Model

The extension model is based on narrow adapter contracts:

- `InputAdapter`
- `RunnerAdapter`
- `RepositoryProvider`
- `PolicyRule`
- `NotificationSink`

Each adapter should be independently testable and registered through configuration.

The first implementation can compile adapters in-process. A future version can move heavy or untrusted adapters out of process if isolation becomes necessary.

## Testing Strategy

Required test layers:

- unit tests for command parsing and policy rules
- adapter contract tests for input, runner, and repository providers
- integration tests for task lifecycle and approvals
- smoke tests for Telegram command handling
- smoke tests for HTTP API task creation
- runner tests with fake Codex and fake shell adapters
- self-maintenance tests for candidate promotion and rollback state transitions

Before any `jin` candidate is promoted, the self-maintenance gate must run:

- build
- unit tests
- integration smoke tests
- migration compatibility checks
- supervisor rollback dry run where possible

## Open Implementation Decisions

These are implementation choices, not unresolved product requirements:

- exact backend language and framework
- exact Telegram library
- exact Codex invocation mode
- whether the first task queue is custom PostgreSQL leasing or a dedicated job library
- whether the first supervisor is a separate binary, launchd service wrapper, or process manager integration

The design assumes those choices will be made in the implementation plan.
