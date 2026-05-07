# Jin Content Factory Design

## Goal

Jin adds a separate Content Factory mode for long-running project-scoped content
pipelines. Factories are not normal chats, but they keep a chat-like timeline,
produce artifacts, request approvals, and can sync with Telegram in both
directions.

## MVP Scope

The MVP implements real pipeline state, API, web surfaces, Telegram sync
settings, and artifact/review data models. Text, script, and image artifacts are
first-class generation targets. 3D and music are reserved artifact/provider
contracts, not required to run in the first MVP.

Direct publication to external channels is out of scope for this MVP.

## Core Model

- `ProjectContentProfile`: one reusable content profile per project with
  audience, language, tone, persona, content pillars, references, constraints,
  and optional publish channel notes.
- `FactoryPipeline`: project-scoped pipeline with brief, mode, review policy,
  content types, schedule, output path, selected sync targets, stages,
  artifacts, review bundles, and timeline events.
- `FactoryStage`: stage type `Brief`, `Research`, `Plan`, `Generate`,
  `Refine`, or `Review` with status `Pending`, `Running`, `WaitingApproval`,
  `Approved`, `NeedsChanges`, `Skipped`, or `Failed`.
- `FactoryArtifact`: typed output `Text`, `Script`, `Image`, `ThreeD`, or
  `Music` with files, preview text/path, provider metadata, and revision count.
- `ReviewBundle`: bundle-level approval with artifact-level decisions and a
  `request_changes` loop.
- `FactoryEvent`: durable chat-like timeline item for web and messenger sync.

Pipeline status:

`Draft -> Scheduled -> Running -> WaitingApproval -> WaitingCapacity -> Paused -> Completed -> Failed -> Stopped`

Pipeline mode:

- `Finite`
- `Continuous`

Review policy:

- `FinalOnly`
- `PerStage`

## Execution

The MVP runs a factory worker embedded inside `jin-server`, but keeps the model
separate enough to move into a future `jin-worker`. The scheduler allows one
active factory pipeline per project. Different projects may run in parallel.

Factories are schedule-aware and capacity-aware. Providers can report capacity
states such as quota exhaustion, rate limits, or temporary unavailability.
Pipelines move to `WaitingCapacity`, notify sync targets, and resume
automatically when capacity is available. Manual stop/pause/resume is always
available from web and Telegram.

## Providers

The provider registry is keyed by artifact type. MVP candidates:

- `codex.text`
- `codex.script`
- `codex.image`
- `custom_cli.image`

Provider contract:

`ArtifactGenerationRequest -> ArtifactGenerationResult`

Requests include project, pipeline, stage, brief, project content profile,
references, constraints, output path, artifact type, and requested count.
Results include files, preview, prompt/spec used, provider metadata, and
diagnostics.

## Web UX

The web client gets a separate `Factories` mode:

- grouped factory list by project;
- new pipeline wizard with a prompt brief and structured fields;
- project content profile editor;
- pipeline detail page with stages, timeline, artifacts, review bundles, and
  stop/pause/resume actions;
- sync-target multi-select at chat and factory creation.

Factories do not create normal `ChatSession` records.

## Telegram And Sync

Jin has a shared Channel Sync Service for chats and factories.

Telegram bot token is configured in global settings. The token is write-only in
the web UI: the API reports whether it is configured, but never returns the
secret value.

Telegram mapping:

- Telegram supergroup with forum topics can represent a Jin project.
- Telegram forum topic can represent a Jin chat or factory pipeline.
- If topics are unavailable, Jin can sync into the group/chat without thread
  separation.

When creating a chat or factory in Jin, the user can choose sync targets from a
multi-select. Defaults come from global settings and project settings. A single
chat/factory may sync to multiple targets.

Two-way sync:

- Jin sends timeline events, stage updates, artifact previews, review cards, and
  status changes to selected targets.
- Telegram messages in a linked chat/topic enter the linked Jin chat/factory
  timeline.
- Telegram commands support status, stop, resume, approval, rejection, and
  request changes.

## Data Safety

Heavy artifacts are stored on disk under a pipeline output path. The default is
inside the project, but a pipeline can override it. Jin state stores metadata,
paths, statuses, approvals, and audit events.

Bot token changes are audited. Future secret storage can replace file-backed
state without changing the API shape.
