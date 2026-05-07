import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { FactoryPipeline, ProjectRecord } from "../api/types";
import { FactoriesView } from "./FactoriesView";

const projects: ProjectRecord[] = [{ name: "jin", root: "/tmp/jin" }];
const telegramTarget = {
  id: "telegram-default",
  label: "Telegram project topic",
  kind: "TelegramForumTopic" as const,
  chat_id: "-10010",
  message_thread_id: 42,
};

const factory: FactoryPipeline = {
  id: "factory-1",
  project: "jin",
  title: "Agent content",
  brief: "Create articles and image concepts",
  mode: "Finite",
  review_policy: "PerStage",
  status: "Draft",
  content_types: ["Text", "Image"],
  output_path: "/tmp/jin/.jin/factories/factory-1",
  schedule: {},
  sync_targets: [],
  stages: [
    { stage_type: "Brief", status: "Pending", revision: 0, notes: null },
    { stage_type: "Research", status: "Pending", revision: 0, notes: null },
  ],
  artifacts: [],
  review_bundles: [],
  events: [
    {
      id: "event-1",
      pipeline_id: "factory-1",
      kind: "System",
      content: "factory pipeline created",
      created_at: "2026-05-07T00:00:00Z",
    },
  ],
  created_at: "2026-05-07T00:00:00Z",
  updated_at: "2026-05-07T00:00:00Z",
};

describe("FactoriesView", () => {
  it("creates a factory pipeline from structured fields", async () => {
    const api = {
      createFactory: vi.fn().mockResolvedValue(factory),
      resumeFactory: vi.fn(),
      pauseFactory: vi.fn(),
      stopFactory: vi.fn(),
    };
    const onChanged = vi.fn();
    render(
      <FactoriesView
        projects={projects}
        factories={[]}
        api={api}
        selectedFactoryId={null}
        defaultSyncTargets={[telegramTarget]}
        onNavigate={vi.fn()}
        onChanged={onChanged}
      />,
    );

    await userEvent.type(screen.getByLabelText("Brief"), "Create articles and image concepts");
    await userEvent.click(screen.getByRole("checkbox", { name: "Image" }));
    await userEvent.click(screen.getByRole("button", { name: "Create factory" }));

    await waitFor(() => expect(api.createFactory).toHaveBeenCalled());
    expect(api.createFactory).toHaveBeenCalledWith({
      project: "jin",
      title: null,
      brief: "Create articles and image concepts",
      mode: "Finite",
      review_policy: "PerStage",
      content_types: ["Text", "Script", "Image"],
      output_path: null,
      sync_targets: [telegramTarget],
    });
    expect(onChanged).toHaveBeenCalled();
  });

  it("renders factory details and lifecycle actions", async () => {
    const api = {
      createFactory: vi.fn(),
      resumeFactory: vi.fn().mockResolvedValue({ ...factory, status: "Scheduled" }),
      pauseFactory: vi.fn(),
      stopFactory: vi.fn(),
    };
    render(
      <FactoriesView
        projects={projects}
        factories={[factory]}
        api={api}
        selectedFactoryId="factory-1"
        defaultSyncTargets={[]}
        onNavigate={vi.fn()}
        onChanged={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Agent content" })).toBeInTheDocument();
    expect(screen.getByText("factory pipeline created")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Resume" }));

    expect(api.resumeFactory).toHaveBeenCalledWith("factory-1");
  });
});
