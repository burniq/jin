export function DocsView() {
  return (
    <section className="docs-view">
      <h1>Jin docs</h1>
      <div className="doc-grid">
        <article>
          <h2>Chats</h2>
          <p>Create a chat, choose a project/tool/model, and control native agent sessions.</p>
        </article>
        <article>
          <h2>Progress</h2>
          <p>The client polls chat progress and appends durable tool output to the timeline.</p>
        </article>
        <article>
          <h2>Settings</h2>
          <p>Model and reasoning are editable when the selected tool exposes those controls.</p>
        </article>
        <article>
          <h2>Factories</h2>
          <p>Create project-scoped content pipelines, choose artifact types, and pause, resume, or stop them.</p>
        </article>
        <article>
          <h2>Telegram sync</h2>
          <p>Configure a write-only bot token and default Telegram chat/topic sync targets for chats and factories.</p>
        </article>
        <article>
          <h2>Public host</h2>
          <p>Global Jin settings define the host used when chat output points to localhost services.</p>
        </article>
        <article>
          <h2>Approvals</h2>
          <p>Risky operations stay approval-gated by the backend policy layer.</p>
        </article>
      </div>
    </section>
  );
}
