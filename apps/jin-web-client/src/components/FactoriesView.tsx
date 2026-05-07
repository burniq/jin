import { Pause, Play, Square, WandSparkles } from "lucide-react";
import { useMemo, useState } from "react";
import type {
  CreateFactoryPayload,
  FactoryArtifactKind,
  FactoryPipeline,
  ProjectRecord,
  SyncTarget,
} from "../api/types";

interface FactoriesViewApi {
  createFactory(payload: CreateFactoryPayload): Promise<FactoryPipeline>;
  pauseFactory(factoryId: string): Promise<FactoryPipeline>;
  resumeFactory(factoryId: string): Promise<FactoryPipeline>;
  stopFactory(factoryId: string): Promise<FactoryPipeline>;
}

interface FactoriesViewProps {
  projects: ProjectRecord[];
  factories: FactoryPipeline[];
  api: FactoriesViewApi;
  selectedFactoryId: string | null;
  defaultSyncTargets: SyncTarget[];
  onNavigate: (path: string) => void;
  onChanged: () => void;
}

const artifactOptions: FactoryArtifactKind[] = ["Text", "Script", "Image", "ThreeD", "Music"];

export function FactoriesView({
  projects,
  factories,
  api,
  selectedFactoryId,
  defaultSyncTargets,
  onNavigate,
  onChanged,
}: FactoriesViewProps) {
  const selectedFactory = factories.find((factory) => factory.id === selectedFactoryId) ?? null;

  if (selectedFactory) {
    return (
      <FactoryDetail
        factory={selectedFactory}
        api={api}
        onChanged={onChanged}
        onBack={() => onNavigate("/factories")}
      />
    );
  }

  return (
    <section className="factories-view">
      <div className="section-header">
        <div>
          <h1>Factories</h1>
          <p className="muted">Project-scoped content pipelines with review and sync.</p>
        </div>
      </div>
      <NewFactoryForm
        projects={projects}
        api={api}
        defaultSyncTargets={defaultSyncTargets}
        onCreated={(factory) => {
          onChanged();
          onNavigate(`/factories/${factory.id}`);
        }}
      />
      <div className="factory-list">
        {factories.length === 0 ? <p className="empty-state">No factories yet</p> : null}
        {factories.map((factory) => (
          <button
            key={factory.id}
            type="button"
            className="factory-list-item"
            onClick={() => onNavigate(`/factories/${factory.id}`)}
          >
            <strong>{factory.title}</strong>
            <span>
              {factory.project} · {factory.status} · {factory.content_types.join(", ")}
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}

function NewFactoryForm({
  projects,
  api,
  defaultSyncTargets,
  onCreated,
}: {
  projects: ProjectRecord[];
  api: FactoriesViewApi;
  defaultSyncTargets: SyncTarget[];
  onCreated: (factory: FactoryPipeline) => void;
}) {
  const [project, setProject] = useState(projects[0]?.name ?? "");
  const [title, setTitle] = useState("");
  const [brief, setBrief] = useState("");
  const [mode, setMode] = useState<CreateFactoryPayload["mode"]>("Finite");
  const [reviewPolicy, setReviewPolicy] = useState<CreateFactoryPayload["review_policy"]>("PerStage");
  const [outputPath, setOutputPath] = useState("");
  const [selectedTypes, setSelectedTypes] = useState<Set<FactoryArtifactKind>>(
    () => new Set(["Text", "Script"]),
  );
  const [selectedSyncTargetIds, setSelectedSyncTargetIds] = useState<Set<string>>(
    () => new Set(defaultSyncTargets.map((target) => target.id)),
  );
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const selectedSyncTargets = useMemo(
    () => defaultSyncTargets.filter((target) => selectedSyncTargetIds.has(target.id)),
    [defaultSyncTargets, selectedSyncTargetIds],
  );

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const factory = await api.createFactory({
        project,
        title: title.trim() || null,
        brief: brief.trim(),
        mode,
        review_policy: reviewPolicy,
        content_types: artifactOptions.filter((kind) => selectedTypes.has(kind)),
        output_path: outputPath.trim() || null,
        sync_targets: selectedSyncTargets,
      });
      onCreated(factory);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create factory");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form className="factory-form" onSubmit={submit}>
      <label>
        <span>Brief</span>
        <textarea
          value={brief}
          onChange={(event) => setBrief(event.target.value)}
          placeholder="Describe the content pipeline..."
          required
        />
      </label>
      <div className="composer-toolbar">
        <label>
          <span>Project</span>
          <select value={project} onChange={(event) => setProject(event.target.value)} required>
            {projects.map((item) => (
              <option key={item.name} value={item.name}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Mode</span>
          <select value={mode} onChange={(event) => setMode(event.target.value as CreateFactoryPayload["mode"])}>
            <option value="Finite">Finite</option>
            <option value="Continuous">Continuous</option>
          </select>
        </label>
        <label>
          <span>Review</span>
          <select
            value={reviewPolicy}
            onChange={(event) =>
              setReviewPolicy(event.target.value as CreateFactoryPayload["review_policy"])
            }
          >
            <option value="PerStage">Per stage</option>
            <option value="FinalOnly">Final only</option>
          </select>
        </label>
      </div>
      <fieldset className="inline-options">
        <legend>Content types</legend>
        {artifactOptions.map((kind) => (
          <label key={kind}>
            <input
              type="checkbox"
              checked={selectedTypes.has(kind)}
              onChange={() =>
                setSelectedTypes((current) => {
                  const next = new Set(current);
                  if (next.has(kind)) {
                    next.delete(kind);
                  } else {
                    next.add(kind);
                  }
                  return next;
                })
              }
            />
            <span>{kind}</span>
          </label>
        ))}
      </fieldset>
      {defaultSyncTargets.length > 0 ? (
        <fieldset className="inline-options">
          <legend>Sync</legend>
          {defaultSyncTargets.map((target) => (
            <label key={target.id}>
              <input
                type="checkbox"
                checked={selectedSyncTargetIds.has(target.id)}
                onChange={() =>
                  setSelectedSyncTargetIds((current) => {
                    const next = new Set(current);
                    if (next.has(target.id)) {
                      next.delete(target.id);
                    } else {
                      next.add(target.id);
                    }
                    return next;
                  })
                }
              />
              <span>{target.label}</span>
            </label>
          ))}
        </fieldset>
      ) : null}
      <details className="secondary-settings">
        <summary>More</summary>
        <label>
          <span>Title</span>
          <input value={title} onChange={(event) => setTitle(event.target.value)} />
        </label>
        <label>
          <span>Output path</span>
          <input value={outputPath} onChange={(event) => setOutputPath(event.target.value)} />
        </label>
      </details>
      <button type="submit" disabled={submitting || !project || !brief.trim() || selectedTypes.size === 0}>
        <WandSparkles size={16} />
        {submitting ? "Creating..." : "Create factory"}
      </button>
      {error ? <p className="form-error">{error}</p> : null}
    </form>
  );
}

function FactoryDetail({
  factory,
  api,
  onChanged,
  onBack,
}: {
  factory: FactoryPipeline;
  api: FactoriesViewApi;
  onChanged: () => void;
  onBack: () => void;
}) {
  const runAction = async (action: "pause" | "resume" | "stop") => {
    if (action === "pause") {
      await api.pauseFactory(factory.id);
    } else if (action === "resume") {
      await api.resumeFactory(factory.id);
    } else {
      await api.stopFactory(factory.id);
    }
    onChanged();
  };

  return (
    <section className="factory-detail">
      <button type="button" className="text-button" onClick={onBack}>
        Back to factories
      </button>
      <div className="section-header">
        <div>
          <h1>{factory.title}</h1>
          <p className="muted">
            {factory.project} · {factory.status} · {factory.mode} · {factory.review_policy}
          </p>
        </div>
        <div className="button-row">
          <button type="button" onClick={() => void runAction("resume")}>
            <Play size={16} />
            Resume
          </button>
          <button type="button" onClick={() => void runAction("pause")}>
            <Pause size={16} />
            Pause
          </button>
          <button type="button" onClick={() => void runAction("stop")}>
            <Square size={16} />
            Stop
          </button>
        </div>
      </div>
      <section className="factory-panel">
        <h2>Brief</h2>
        <p>{factory.brief}</p>
      </section>
      <section className="factory-panel">
        <h2>Stages</h2>
        <div className="stage-list">
          {factory.stages.map((stage) => (
            <span key={stage.stage_type} className="stage-pill">
              {stage.stage_type}: {stage.status}
            </span>
          ))}
        </div>
      </section>
      <section className="factory-panel">
        <h2>Timeline</h2>
        {factory.events.length === 0 ? <p className="muted">No events yet</p> : null}
        {factory.events.map((event) => (
          <article key={event.id} className="factory-event">
            <strong>{event.kind}</strong>
            <p>{event.content}</p>
          </article>
        ))}
      </section>
    </section>
  );
}
