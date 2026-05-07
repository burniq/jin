import { BookOpen, ChevronRight, Factory, FolderKanban, Menu, Plus, Settings, ShieldCheck, X } from "lucide-react";
import { useState } from "react";
import type { ChatSession, ProjectRecord } from "../api/types";

interface AppShellProps {
  chats: ChatSession[];
  projects: ProjectRecord[];
  children: React.ReactNode;
  onNavigate: (path: string) => void;
}

export function AppShell({ chats, projects, children, onNavigate }: AppShellProps) {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [collapsedProjects, setCollapsedProjects] = useState<Set<string>>(() => readCollapsedProjects());
  const projectGroups = groupChatsByProject(projects, chats);

  const navigate = (path: string) => {
    setDrawerOpen(false);
    onNavigate(path);
  };

  const toggleProject = (projectName: string) => {
    setCollapsedProjects((current) => {
      const next = new Set(current);
      if (next.has(projectName)) {
        next.delete(projectName);
      } else {
        next.add(projectName);
      }
      writeCollapsedProjects(next);
      return next;
    });
  };

  return (
    <div className="app-shell">
      <header className="mobile-topbar">
        <button type="button" aria-label="Open chats" onClick={() => setDrawerOpen(true)}>
          <Menu size={18} />
        </button>
        <strong>jin</strong>
        <button type="button" aria-label="Open navigation" onClick={() => setDrawerOpen(true)}>
          <BookOpen size={18} />
        </button>
      </header>
      <aside className={`sidebar ${drawerOpen ? "open" : ""}`}>
        <div className="brand-row">
          <strong>jin</strong>
          <button type="button" className="mobile-only" aria-label="Close chats" onClick={() => setDrawerOpen(false)}>
            <X size={18} />
          </button>
        </div>
        <button type="button" className="new-chat-button" onClick={() => navigate("/")}>
          <Plus size={16} />
          New chat
        </button>
        <nav className="chat-list" aria-label="Chats">
          {projectGroups.map((group) => (
            <section key={group.project.name} className="project-chat-group">
              <div className="project-chat-heading-row">
                <button
                  type="button"
                  className="project-chat-heading"
                  aria-expanded={!collapsedProjects.has(group.project.name)}
                  onClick={() => toggleProject(group.project.name)}
                >
                  <ChevronRight
                    size={14}
                    className={collapsedProjects.has(group.project.name) ? "" : "expanded"}
                    aria-hidden="true"
                  />
                  <span className="project-chat-heading-text">
                    <span className="project-chat-name">{group.project.name}</span>
                    <span className="project-chat-root">{group.project.root}</span>
                  </span>
                </button>
                <button
                  type="button"
                  className="project-new-chat-button"
                  aria-label={`New chat in ${group.project.name}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    navigate(`/?project=${encodeURIComponent(group.project.name)}`);
                  }}
                >
                  <Plus size={14} aria-hidden="true" />
                </button>
              </div>
              {!collapsedProjects.has(group.project.name) ? (
                <>
                  {group.chats.length === 0 ? <p>No chats yet</p> : null}
                  {group.chats.map((chat) => (
                    <button
                      key={chat.id}
                      type="button"
                      className="chat-list-item"
                      onClick={() => navigate(`/chats/${chat.id}`)}
                    >
                      <strong>{chat.title}</strong>
                      <span>
                        {chat.tool} · {chat.status}
                      </span>
                    </button>
                  ))}
                </>
              ) : null}
            </section>
          ))}
        </nav>
        <nav className="global-nav" aria-label="Navigation">
          <button type="button" onClick={() => navigate("/factories")}>
            <Factory size={16} />
            Factories
          </button>
          <button type="button" onClick={() => navigate("/projects")}>
            <FolderKanban size={16} />
            Projects
          </button>
          <button type="button" onClick={() => navigate("/approvals")}>
            <ShieldCheck size={16} />
            Approvals
          </button>
          <button type="button" onClick={() => navigate("/docs")}>
            <BookOpen size={16} />
            Docs
          </button>
          <button type="button" onClick={() => navigate("/settings")}>
            <Settings size={16} />
            Settings
          </button>
        </nav>
      </aside>
      {drawerOpen ? <button className="scrim" aria-label="Close chats" onClick={() => setDrawerOpen(false)} /> : null}
      <main className="main-surface">{children}</main>
    </div>
  );
}

function groupChatsByProject(projects: ProjectRecord[], chats: ChatSession[]) {
  const groups = projects.map((project) => ({
    project,
    chats: chats.filter((chat) => chat.project === project.name),
  }));
  const knownProjects = new Set(projects.map((project) => project.name));
  const missingProjectNames = Array.from(
    new Set(chats.filter((chat) => !knownProjects.has(chat.project)).map((chat) => chat.project)),
  );
  for (const projectName of missingProjectNames) {
    groups.push({
      project: { name: projectName, root: projectName },
      chats: chats.filter((chat) => chat.project === projectName),
    });
  }
  return groups;
}

const COLLAPSED_PROJECTS_KEY = "jin.sidebar.collapsedProjects";

function readCollapsedProjects() {
  try {
    const raw = window.localStorage.getItem(COLLAPSED_PROJECTS_KEY);
    const value = raw ? JSON.parse(raw) : [];
    return new Set<string>(Array.isArray(value) ? value.filter((item) => typeof item === "string") : []);
  } catch {
    return new Set<string>();
  }
}

function writeCollapsedProjects(collapsedProjects: Set<string>) {
  window.localStorage.setItem(COLLAPSED_PROJECTS_KEY, JSON.stringify(Array.from(collapsedProjects)));
}
