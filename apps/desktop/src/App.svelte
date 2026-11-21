<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { api } from "./lib/api";
  import Icon from "./lib/Icon.svelte";
  import StatusBadge from "./lib/StatusBadge.svelte";
  import type {
    AppError,
    AppInfo,
    InstallProgress,
    LogLine,
    Project,
    ProjectDetail,
    ProjectFilter,
    RunLogEvent,
    RunStatusEvent,
    View,
  } from "./lib/types";

  let projects = $state<Project[]>([]);
  let detail = $state<ProjectDetail | null>(null);
  let activeProjectId = $state<string | null>(null);
  let activeRunId = $state<string | null>(null);
  let logs = $state<LogLine[]>([]);
  let logFilter = $state<"all" | "stdout" | "stderr" | "system">("all");
  let appInfo = $state<AppInfo | null>(null);
  let view = $state<View>("library");
  let filter = $state<ProjectFilter>("all");
  let search = $state("");
  let loading = $state(true);
  let addOpen = $state(false);
  let url = $state("");
  let adding = $state(false);
  let busyLabel = $state("");
  let addError = $state<AppError | null>(null);
  let error = $state<AppError | null>(null);
  let toast = $state("");
  let catalogChannel = $state<"official" | "community">("official");
  let dockerRefreshing = $state(false);
  let nativeWarningAccepted = $state(false);
  let confirmRequest = $state<ConfirmRequest | null>(null);
  let logBox = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);
  let toastTimer: number | undefined;

  type ConfirmRequest = {
    title: string;
    message: string;
    confirmLabel: string;
    danger: boolean;
    resolve: (value: boolean) => void;
  };

  const activeStates = ["starting", "installing", "running", "stopping"];

  let visibleProjects = $derived(
    projects.filter((project) => {
      const query = search.trim().toLowerCase();
      const matchesQuery =
        !query ||
        project.name.toLowerCase().includes(query) ||
        (project.owner ?? "").toLowerCase().includes(query) ||
        (project.repo ?? "").toLowerCase().includes(query);
      const matchesFilter =
        filter === "all" ||
        (filter === "running" && activeStates.includes(project.state)) ||
        (filter === "stopped" &&
          ["ready", "stopped", "interrupted"].includes(project.state)) ||
        (filter === "error" && project.state === "error");
      return matchesQuery && matchesFilter;
    }),
  );

  let runningCount = $derived(
    projects.filter((project) => activeStates.includes(project.state)).length,
  );
  let stoppedCount = $derived(
    projects.filter((project) =>
      ["ready", "stopped", "interrupted"].includes(project.state),
    ).length,
  );
  let errorCount = $derived(
    projects.filter((project) => project.state === "error").length,
  );
  let currentTitle = $derived(
    activeProjectId && detail
      ? detail.project.name
      : view === "library"
        ? "Seus projetos"
        : view === "catalog"
          ? "Catálogo"
          : "Configurações",
  );
  let currentEyebrow = $derived(
    activeProjectId && detail
      ? "Projeto"
      : view === "library"
        ? "Biblioteca"
        : view === "catalog"
          ? "Marketplace"
          : "Preferências",
  );
  let logPort = $derived.by(() => {
    for (let index = logs.length - 1; index >= 0; index -= 1) {
      const match = logs[index].line.match(
        /https?:\/\/(?:localhost|127\.0\.0\.1|\[::1\]):(\d{2,5})/i,
      );
      if (match) return Number(match[1]);
    }
    return null;
  });
  let openPort = $derived(
    logPort ?? detail?.plan?.ports?.[0]?.container ?? null,
  );
  let runActive = $derived(
    detail ? activeStates.includes(detail.project.state) : false,
  );
  let canRun = $derived(
    Boolean(detail?.plan && detail.plan.start.length > 0),
  );
  let filteredLogs = $derived(
    logFilter === "all"
      ? logs
      : logs.filter((line) => line.stream === logFilter),
  );
  let logCounts = $derived({
    all: logs.length,
    stdout: logs.filter((line) => line.stream === "stdout").length,
    stderr: logs.filter((line) => line.stream === "stderr").length,
    system: logs.filter((line) => line.stream === "system").length,
  });

  onMount(() => {
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    const registerEvents = async () => {
      try {
        const [unLog, unStatus, unProgress] = await Promise.all([
          listen<RunLogEvent>("run-log", (event) => appendLog(event.payload)),
          listen<RunStatusEvent>("run-status", (event) =>
            applyStatus(event.payload),
          ),
          listen<InstallProgress>("install-progress", (event) => {
            busyLabel = event.payload.message;
          }),
        ]);
        if (cancelled) {
          unLog();
          unStatus();
          unProgress();
        } else {
          unlisteners.push(unLog, unStatus, unProgress);
        }
      } catch (eventError) {
        console.warn("Eventos do Tauri indisponíveis:", eventError);
      }
    };

    void registerEvents();
    void loadApp();

    return () => {
      cancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  });

  $effect(() => {
    if (logs.length > 0 && logBox) {
      logBox.scrollTop = logBox.scrollHeight;
    }
  });

  async function loadApp() {
    loading = true;
    try {
      const [info, loadedProjects] = await Promise.all([
        api.appInfo(),
        api.listProjects(),
      ]);
      appInfo = info;
      projects = loadedProjects;
    } catch (loadError) {
      setError(loadError);
    } finally {
      loading = false;
    }
  }

  async function refreshProjects() {
    projects = await api.listProjects();
    if (
      activeProjectId &&
      !projects.some((project) => project.id === activeProjectId)
    ) {
      backToLibrary();
    }
  }

  async function openProject(projectId: string) {
    view = "library";
    activeProjectId = projectId;
    logs = [];
    activeRunId = null;
    error = null;
    try {
      detail = await api.projectDetail(projectId);
      const latestRun = detail.runs[0];
      if (latestRun) {
        activeRunId = latestRun.id;
        logs = await api.readLog(projectId, latestRun.id);
      }
      await tickLogs();
    } catch (openError) {
      setError(openError);
    }
  }

  async function refreshDetail(projectId: string) {
    if (activeProjectId !== projectId) return;
    try {
      detail = await api.projectDetail(projectId);
    } catch (detailError) {
      setError(detailError);
    }
  }

  function appendLog(event: RunLogEvent) {
    if (event.projectId !== activeProjectId) return;
    if (activeRunId && event.runId !== activeRunId) return;
    if (!activeRunId) activeRunId = event.runId;
    logs = [...logs, { stream: event.stream, line: event.line }];
    if (logs.length > 5000) {
      logs = logs.slice(logs.length - 5000);
    }
  }

  function applyStatus(event: RunStatusEvent) {
    applyLocalState(event.projectId, event.state);
    if (event.projectId === activeProjectId && !activeRunId) {
      activeRunId = event.runId;
    }
    if (event.state === "error" && event.message) {
      showToast(event.message);
      void refreshDetail(event.projectId);
    }
    if (["stopped", "interrupted"].includes(event.state)) {
      void refreshDetail(event.projectId);
    }
    if (event.state === "running") {
      void refreshDetail(event.projectId);
    }
  }

  function applyLocalState(projectId: string, state: string) {
    projects = projects.map((project) =>
      project.id === projectId ? { ...project, state } : project,
    );
    if (detail?.project.id === projectId) {
      detail = {
        ...detail,
        project: { ...detail.project, state },
      };
    }
  }

  async function addProject() {
    if (adding) return;
    const value = url.trim();
    if (!value) return;
    adding = true;
    addError = null;
    busyLabel = "Iniciando download...";
    try {
      const created = await api.addProject(value);
      addOpen = false;
      url = "";
      await refreshProjects();
      await openProject(created.project.id);
      showToast("Projeto instalado. Revise o plano e execute.");
    } catch (addFailure) {
      addError = normalizeError(addFailure);
    } finally {
      adding = false;
      busyLabel = "";
    }
  }

  function askConfirm(
    title: string,
    message: string,
    confirmLabel = "Continuar",
    danger = false,
  ) {
    return new Promise<boolean>((resolve) => {
      confirmRequest = { title, message, confirmLabel, danger, resolve };
    });
  }

  function resolveConfirm(value: boolean) {
    const request = confirmRequest;
    confirmRequest = null;
    request?.resolve(value);
  }

  async function startSelected() {
    if (!detail) return;
    if (!nativeWarningAccepted) {
      const accepted = await askConfirm(
        "Execução nativa",
        "O projeto rodará com as permissões do seu usuário e poderá acessar arquivos e rede. O isolamento com Docker entra na fase 2. Revise os comandos antes de continuar.",
        "Executar mesmo assim",
      );
      if (!accepted) return;
      nativeWarningAccepted = true;
    }
    error = null;
    logs = [];
    activeRunId = null;
    try {
      const run = await api.startProject(detail.project.id);
      activeRunId = run.runId;
      applyLocalState(detail.project.id, "starting");
      showToast("Execução iniciada.");
    } catch (startError) {
      setError(startError);
    }
  }

  async function startById(projectId: string) {
    if (!nativeWarningAccepted) {
      const accepted = await askConfirm(
        "Execução nativa",
        "O projeto rodará com as permissões do seu usuário e poderá acessar arquivos e rede. O isolamento com Docker entra na fase 2. Revise os comandos antes de continuar.",
        "Executar mesmo assim",
      );
      if (!accepted) return;
      nativeWarningAccepted = true;
    }
    try {
      if (activeProjectId === projectId) {
        logs = [];
        activeRunId = null;
      }
      const run = await api.startProject(projectId);
      if (activeProjectId === projectId) {
        activeRunId = run.runId;
      }
      applyLocalState(projectId, "starting");
      showToast("Execução iniciada.");
    } catch (startError) {
      setError(startError);
    }
  }

  async function stopSelected() {
    if (!detail) return;
    try {
      await api.stopProject(detail.project.id);
      applyLocalState(detail.project.id, "stopping");
    } catch (stopError) {
      setError(stopError);
    }
  }

  async function stopById(projectId: string) {
    try {
      await api.stopProject(projectId);
      applyLocalState(projectId, "stopping");
    } catch (stopError) {
      setError(stopError);
    }
  }

  async function removeSelected(deleteFiles: boolean) {
    if (!detail) return;
    const project = detail.project;
    const message = deleteFiles
      ? `Apagar "${project.name}" do YourGit e excluir a pasta ${project.localPath}? Essa ação não pode ser desfeita.`
      : `Remover "${project.name}" do YourGit? Os arquivos em ${project.localPath} serão mantidos.`;
    const accepted = await askConfirm(
      deleteFiles ? "Apagar projeto" : "Remover projeto",
      message,
      deleteFiles ? "Apagar tudo" : "Remover",
      deleteFiles,
    );
    if (!accepted) return;
    try {
      await api.removeProject(project.id, deleteFiles);
      backToLibrary();
      await refreshProjects();
      showToast(deleteFiles ? "Projeto e arquivos removidos." : "Projeto removido.");
    } catch (removeError) {
      setError(removeError);
    }
  }

  async function openApp() {
    if (!openPort) return;
    try {
      await api.openUrl(`http://localhost:${openPort}`);
    } catch (openError) {
      setError(openError);
    }
  }

  async function openFolder() {
    if (!detail) return;
    try {
      await api.openProjectFolder(detail.project.id);
    } catch (openError) {
      setError(openError);
    }
  }

  async function resetDependencies() {
    if (!detail) return;
    try {
      await api.resetDependencies(detail.project.id);
      detail = {
        ...detail,
        project: { ...detail.project, depsReady: false, state: "ready" },
      };
      applyLocalState(detail.project.id, "ready");
      showToast("As dependências serão reinstaladas na próxima execução.");
    } catch (resetError) {
      setError(resetError);
    }
  }

  async function copyLogs() {
    try {
      await navigator.clipboard.writeText(
        filteredLogs.map((line) => `[${line.stream}] ${line.line}`).join("\n"),
      );
      showToast("Logs copiados.");
    } catch {
      showToast("Não foi possível copiar os logs.");
    }
  }

  async function refreshDocker() {
    dockerRefreshing = true;
    try {
      const status = await api.dockerStatus();
      if (appInfo) {
        appInfo = {
          ...appInfo,
          dockerAvailable: status.available,
          dockerVersion: status.version,
        };
      }
      showToast(
        status.available
          ? `Docker ${status.version ?? ""} disponível.`
          : "Docker não está disponível.",
      );
    } catch (dockerError) {
      setError(dockerError);
    } finally {
      dockerRefreshing = false;
    }
  }

  function selectView(nextView: View) {
    view = nextView;
    activeProjectId = null;
    detail = null;
    logs = [];
    error = null;
  }

  function backToLibrary() {
    activeProjectId = null;
    detail = null;
    logs = [];
    activeRunId = null;
    logFilter = "all";
    view = "library";
  }

  function setError(value: unknown) {
    error = normalizeError(value);
  }

  function normalizeError(value: unknown): AppError {
    if (value && typeof value === "object" && "message" in value) {
      const candidate = value as Partial<AppError>;
      return {
        code: candidate.code ?? "YOURGIT-5001",
        title: candidate.title ?? "Erro inesperado",
        message: String(candidate.message ?? "Tente novamente."),
        details: candidate.details ?? null,
        hint: candidate.hint ?? null,
        retryable: candidate.retryable ?? false,
      };
    }
    return {
      code: "YOURGIT-5001",
      title: "Erro inesperado",
      message: String(value),
      details: null,
      hint: null,
      retryable: false,
    };
  }

  function showToast(message: string) {
    toast = message;
    if (toastTimer) window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => {
      toast = "";
    }, 4500);
  }

  function handleShortcut(event: KeyboardEvent) {
    if (confirmRequest) {
      if (event.key === "Escape") {
        event.preventDefault();
        resolveConfirm(false);
      }
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      searchInput?.focus();
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "n") {
      event.preventDefault();
      if (!adding) addOpen = true;
    }
    if (event.key === "Escape") {
      if (addOpen && !adding) {
        addOpen = false;
      } else if (activeProjectId) {
        backToLibrary();
      }
    }
  }

  function commandText(command: string[]) {
    return command
      .map((part) => (part.includes(" ") ? `"${part}"` : part))
      .join(" ");
  }

  function formatDate(value: string | null | undefined) {
    if (!value) return "—";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat("pt-BR", {
      dateStyle: "short",
      timeStyle: "short",
    }).format(date);
  }

  function confidenceLabel(value: number) {
    return `${Math.round(value * 100)}%`;
  }

  async function tickLogs() {
    await new Promise((resolve) => setTimeout(resolve, 0));
    if (logBox) logBox.scrollTop = logBox.scrollHeight;
  }
</script>

<svelte:window onkeydown={handleShortcut} />

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">
        <img src="/brand/app-icon.svg" alt="YourGit" />
      </div>
      <div class="brand-copy">
        <strong>YourGit</strong>
        <span>project runner</span>
      </div>
    </div>

    <nav class="primary-nav" aria-label="Navegação principal">
      <button
        class="nav-item"
        class:active={view === "library"}
        onclick={() => selectView("library")}
      >
        <Icon name="box" />
        <span>Biblioteca</span>
        <span class="nav-count">{projects.length}</span>
      </button>
      <button
        class="nav-item"
        class:active={view === "catalog"}
        onclick={() => selectView("catalog")}
      >
        <Icon name="book" />
        <span>Catálogo</span>
        <span class="nav-badge">2</span>
      </button>
      <button
        class="nav-item"
        class:active={view === "settings"}
        onclick={() => selectView("settings")}
      >
        <Icon name="settings" />
        <span>Configurações</span>
      </button>
    </nav>

    {#if projects.length > 0}
      <div class="side-section">
        <div class="side-label">Projetos</div>
        {#each projects.slice(0, 6) as project (project.id)}
          <button
            class="side-project"
            class:selected={activeProjectId === project.id}
            onclick={() => void openProject(project.id)}
            title={`${project.owner ?? ""}/${project.repo ?? project.name}`}
          >
            <span class="mini-avatar"
              >{project.name.slice(0, 1).toUpperCase()}</span
            >
            <span class="side-project-name">{project.name}</span>
            <span class="mini-state {project.state}"></span>
          </button>
        {/each}
      </div>
    {/if}

    <div class="sidebar-footer">
      <div class="docker-line">
        <span
          class="docker-dot"
          class:online={appInfo?.dockerAvailable}
        ></span>
        <span>Docker {appInfo?.dockerAvailable ? "ativo" : "indisponível"}</span>
      </div>
      <div class="version-line">
        v{appInfo?.version ?? "0.1.0"} · execução nativa
      </div>
    </div>
  </aside>

  <main class="main">
    <header class="topbar">
      <div class="topbar-title">
        {#if activeProjectId && detail}
          <button
            class="icon-button back-button"
            title="Voltar para a biblioteca"
            onclick={backToLibrary}
          >
            <Icon name="arrow-left" />
          </button>
        {/if}
        <div>
          <span class="eyebrow">{currentEyebrow}</span>
          <h1>{currentTitle}</h1>
        </div>
      </div>

      <div class="topbar-actions">
        {#if view === "library" && !activeProjectId}
          <label class="search">
            <Icon name="search" size={16} />
            <input
              bind:this={searchInput}
              bind:value={search}
              placeholder="Buscar projetos...  Ctrl K"
              spellcheck="false"
            />
          </label>
        {/if}
        <button class="button primary" onclick={() => (addOpen = true)}>
          <Icon name="plus" size={16} />
          Adicionar
        </button>
      </div>
    </header>

    {#if error}
      <div class="error-banner" role="alert">
        <div class="error-icon"><Icon name="warning" size={20} /></div>
        <div class="error-copy">
          <div class="error-title">
            <strong>{error.title}</strong>
            <span class="error-code">{error.code}</span>
          </div>
          <p>{error.message}</p>
          {#if error.hint}<p class="error-hint">{error.hint}</p>{/if}
          {#if error.details}
            <details>
              <summary>Detalhes técnicos</summary>
              <pre>{error.details}</pre>
            </details>
          {/if}
        </div>
        <button
          class="icon-button"
          aria-label="Fechar erro"
          onclick={() => (error = null)}
        >
          <Icon name="x" size={16} />
        </button>
      </div>
    {/if}

    <div class="content">
      {#if view === "library"}
        {#if activeProjectId && detail}
          <div class="detail-view">
            <section class="detail-hero">
              <div class="detail-identity">
                <div class="hero-avatar">
                  {detail.project.name.slice(0, 1).toUpperCase()}
                </div>
                <div class="hero-copy">
                  <div class="hero-title">
                    <h2>{detail.project.name}</h2>
                    <StatusBadge state={detail.project.state} />
                  </div>
                  <p class="hero-subtitle">
                    {detail.project.owner ?? "local"}/{detail.project.repo ??
                      detail.project.name}
                    {#if detail.project.requestedRef}
                      <span class="inline-sep">·</span>
                      <span class="mono">{detail.project.requestedRef}</span>
                    {/if}
                  </p>
                  <div class="hero-tags">
                    {#if detail.plan}
                      <span class="tag accent">{detail.plan.stack.language}</span>
                      {#if detail.plan.stack.framework}
                        <span class="tag">{detail.plan.stack.framework}</span>
                      {/if}
                    {/if}
                    <span class="tag">nativo</span>
                    {#if detail.project.depsReady}
                      <span class="tag ok">dependências prontas</span>
                    {:else}
                      <span class="tag warn">deps pendentes</span>
                    {/if}
                  </div>
                </div>
              </div>
              <div class="hero-actions">
                {#if openPort}
                  <button
                    class="button"
                    disabled={!runActive}
                    title={runActive
                      ? "Abrir no navegador"
                      : "Execute o projeto para abrir a porta"}
                    onclick={() => void openApp()}
                  >
                    <Icon name="external" size={16} />
                    Abrir :{openPort}
                  </button>
                {/if}
                {#if runActive}
                  <button
                    class="button danger"
                    onclick={() => void stopSelected()}
                  >
                    <Icon name="stop" size={16} />
                    Parar
                  </button>
                {:else}
                  <button
                    class="button primary"
                    disabled={!canRun}
                    onclick={() => void startSelected()}
                    title={canRun
                      ? "Executar projeto"
                      : "O plano não tem comando de start"}
                  >
                    <Icon name="play" size={16} />
                    Executar
                  </button>
                {/if}
                <button
                  class="icon-button"
                  title="Abrir pasta do projeto"
                  onclick={() => void openFolder()}
                >
                  <Icon name="folder" />
                </button>
              </div>
            </section>

            <div class="detail-grid">
              <div class="detail-main">
                <section class="panel">
                  <header class="panel-head">
                    <div>
                      <h3>Plano de execução</h3>
                      <p>Comandos detectados e revisáveis antes de rodar.</p>
                    </div>
                    {#if detail.plan}
                      <span class="hash-pill" title={detail.plan.planHash}
                        >{detail.plan.planHash.slice(0, 15)}…</span
                      >
                    {/if}
                  </header>

                  {#if detail.plan}
                    <div class="plan-stack">
                      <div class="plan-stat">
                        <span>Stack</span>
                        <strong
                          >{detail.plan.stack.language}
                          {detail.plan.stack.framework
                            ? `· ${detail.plan.stack.framework}`
                            : ""}</strong
                        >
                      </div>
                      <div class="plan-stat">
                        <span>Confiança</span>
                        <strong
                          >{confidenceLabel(
                            detail.plan.stack.confidence,
                          )}</strong
                        >
                      </div>
                      <div class="plan-stat">
                        <span>Workdir</span>
                        <strong class="mono">{detail.plan.workdir}</strong>
                      </div>
                      <div class="plan-stat">
                        <span>Porta</span>
                        <strong class="mono">{openPort ?? "—"}</strong>
                      </div>
                    </div>

                    <div class="command-groups">
                      <div class="command-group">
                        <div class="command-label">
                          <Icon name="download" size={14} /> Instalar
                        </div>
                        {#each detail.plan.install as command}
                          <code class="command">{commandText(command)}</code>
                        {:else}
                          <span class="muted small"
                            >Nenhuma etapa de instalação.</span
                          >
                        {/each}
                      </div>
                      <div class="command-group">
                        <div class="command-label">
                          <Icon name="play" size={14} /> Executar
                        </div>
                        {#each detail.plan.start as command}
                          <code class="command">{commandText(command)}</code>
                        {:else}
                          <span class="muted small"
                            >Nenhum comando de execução detectado. Edite o
                            plano para continuar.</span
                          >
                        {/each}
                      </div>
                    </div>

                    {#if detail.plan.warnings.length > 0}
                      <div class="warnings">
                        {#each detail.plan.warnings as warning}
                          <div class="warning-row">
                            <Icon name="warning" size={15} />
                            <span>{warning}</span>
                          </div>
                        {/each}
                      </div>
                    {/if}
                  {:else}
                    <p class="muted">Este projeto não tem um plano salvo.</p>
                  {/if}

                  <div class="callout warn native-callout">
                    <Icon name="shield" size={18} />
                    <div>
                      <strong>Execução nativa</strong>
                      <p>
                        O código roda com as permissões do seu usuário. O
                        isolamento em Docker está planejado para a fase 2; até
                        lá, revise os comandos e use apenas repositórios
                        confiáveis.
                      </p>
                    </div>
                  </div>

                  <div class="plan-actions">
                    <button
                      class="button ghost"
                      onclick={() => void resetDependencies()}
                      disabled={runActive}
                    >
                      <Icon name="refresh" size={15} />
                      Reinstalar dependências
                    </button>
                    <span class="muted small">
                      {detail.project.depsReady
                        ? "Dependências já instaladas."
                        : "Serão instaladas na primeira execução."}
                    </span>
                  </div>
                </section>

                <section class="panel logs-panel">
                  <header class="panel-head logs-head">
                    <div>
                      <h3>Logs</h3>
                      <p>
                        {runActive
                          ? "Stream em tempo real."
                          : "Última execução registrada."}
                      </p>
                    </div>
                    <div class="log-tools">
                      <div class="log-tabs">
                        {#each ["all", "stdout", "stderr", "system"] as tab}
                          <button
                            class="log-tab"
                            class:active={logFilter === tab}
                            onclick={() =>
                              (logFilter = tab as
                                | "all"
                                | "stdout"
                                | "stderr"
                                | "system")}
                          >
                            {tab === "all" ? "Todos" : tab}
                            <span>{logCounts[tab as keyof typeof logCounts]}</span>
                          </button>
                        {/each}
                      </div>
                      <button
                        class="icon-button"
                        title="Copiar logs"
                        onclick={() => void copyLogs()}
                      >
                        <Icon name="copy" size={15} />
                      </button>
                      <button
                        class="icon-button"
                        title="Limpar visualização"
                        onclick={() => (logs = [])}
                      >
                        <Icon name="trash" size={15} />
                      </button>
                    </div>
                  </header>

                  <div class="log-box" bind:this={logBox} aria-live="polite">
                    {#if filteredLogs.length === 0}
                      <div class="log-empty">
                        <Icon name="terminal" size={22} />
                        <span
                          >Nenhum log ainda. Clique em Executar para
                          começar.</span
                        >
                      </div>
                    {:else}
                      {#each filteredLogs as line, index (index)}
                        <div class="log-line {line.stream}">
                          <span class="log-stream">{line.stream}</span>
                          <span class="log-text">{line.line}</span>
                        </div>
                      {/each}
                    {/if}
                  </div>
                </section>

                <section class="panel">
                  <header class="panel-head">
                    <div>
                      <h3>Histórico</h3>
                      <p>Execuções mais recentes deste projeto.</p>
                    </div>
                  </header>
                  {#if detail.runs.length === 0}
                    <p class="muted small">Ainda não há execuções.</p>
                  {:else}
                    <div class="run-list">
                      {#each detail.runs as run (run.id)}
                        <div class="run-row">
                          <StatusBadge state={run.state} />
                          <span class="mono small">{run.mode}</span>
                          <span class="run-date">{formatDate(run.startedAt)}</span>
                          <span class="run-exit">
                            {run.exitCode === null
                              ? "—"
                              : `exit ${run.exitCode}`}
                          </span>
                        </div>
                      {/each}
                    </div>
                  {/if}
                </section>
              </div>

              <aside class="detail-side">
                <section class="panel">
                  <header class="panel-head compact">
                    <h3>Origem</h3>
                  </header>
                  <dl class="meta-list">
                    <div>
                      <dt>Repositório</dt>
                      <dd class="truncate" title={detail.project.sourceUrl}
                        >{detail.project.sourceUrl}</dd
                      >
                    </div>
                    <div>
                      <dt>Ref</dt>
                      <dd class="mono">{detail.project.requestedRef ?? "padrão"}</dd>
                    </div>
                    <div>
                      <dt>Pasta</dt>
                      <dd class="mono">{detail.project.subdir}</dd>
                    </div>
                    <div>
                      <dt>Criado em</dt>
                      <dd>{formatDate(detail.project.createdAt)}</dd>
                    </div>
                  </dl>
                  <button
                    class="button ghost full"
                    onclick={() => void openFolder()}
                  >
                    <Icon name="folder" size={15} />
                    Abrir pasta
                  </button>
                </section>

                <section class="panel">
                  <header class="panel-head compact">
                    <h3>Porta</h3>
                  </header>
                  {#if openPort}
                    <div class="port-display">
                      <span class="port-number">{openPort}</span>
                      <span class="muted small">localhost</span>
                    </div>
                    <button
                      class="button full"
                      disabled={!runActive}
                      onclick={() => void openApp()}
                    >
                      <Icon name="external" size={15} />
                      Abrir no navegador
                    </button>
                  {:else}
                    <p class="muted small">Nenhuma porta detectada no plano.</p>
                  {/if}
                </section>

                <section class="panel">
                  <header class="panel-head compact">
                    <h3>Ambiente</h3>
                    <span class="count-pill"
                      >{detail.plan?.env.length ?? 0}</span
                    >
                  </header>
                  {#if detail.plan && detail.plan.env.length > 0}
                    <div class="env-list">
                      {#each detail.plan.env as variable (variable.key)}
                        <div class="env-row">
                          <div>
                            <code>{variable.key}</code>
                            {#if variable.description}
                              <span class="env-description"
                                >{variable.description}</span
                              >
                            {/if}
                          </div>
                          {#if variable.secret}
                            <span class="tag lock"
                              ><Icon name="lock" size={11} /> segredo</span
                            >
                          {:else if variable.required}
                            <span class="tag warn">obrigatória</span>
                          {/if}
                        </div>
                      {/each}
                    </div>
                  {:else}
                    <p class="muted small">
                      Nenhuma variável detectada em .env.example.
                    </p>
                  {/if}
                  <p class="panel-note">
                    Edição de variáveis e cofre de segredos entram na próxima
                    fase.
                  </p>
                </section>

                <section class="panel danger-panel">
                  <header class="panel-head compact">
                    <h3>Zona de perigo</h3>
                  </header>
                  <p class="muted small">
                    Remover do YourGit mantém o código no disco. Apagar tudo
                    exclui a pasta do projeto.
                  </p>
                  <button
                    class="button ghost full"
                    disabled={runActive}
                    onclick={() => void removeSelected(false)}
                  >
                    <Icon name="x" size={15} />
                    Remover do YourGit
                  </button>
                  <button
                    class="button danger full"
                    disabled={runActive}
                    onclick={() => void removeSelected(true)}
                  >
                    <Icon name="trash" size={15} />
                    Apagar projeto e arquivos
                  </button>
                </section>
              </aside>
            </div>
          </div>
        {:else}
          <section class="library-head">
            <div>
              <h2>Biblioteca</h2>
              <p>
                Projetos baixados e prontos para executar em um clique.
              </p>
            </div>
            <div class="filter-row">
              <button
                class="filter-chip"
                class:active={filter === "all"}
                onclick={() => (filter = "all")}
                >Todos <span>{projects.length}</span></button
              >
              <button
                class="filter-chip"
                class:active={filter === "running"}
                onclick={() => (filter = "running")}
                >Rodando <span>{runningCount}</span></button
              >
              <button
                class="filter-chip"
                class:active={filter === "stopped"}
                onclick={() => (filter = "stopped")}
                >Parados <span>{stoppedCount}</span></button
              >
              <button
                class="filter-chip"
                class:active={filter === "error"}
                onclick={() => (filter = "error")}
                >Erros <span>{errorCount}</span></button
              >
            </div>
          </section>

          {#if loading}
            <div class="project-grid">
              {#each [1, 2, 3] as item}
                <div class="project-card skeleton" aria-hidden="true">
                  <div class="skeleton-line wide"></div>
                  <div class="skeleton-line"></div>
                  <div class="skeleton-line short"></div>
                </div>
              {/each}
            </div>
          {:else if visibleProjects.length === 0}
            <section class="empty-state">
              <div class="empty-orbit">
                <div class="empty-icon"><Icon name="github" size={34} /></div>
              </div>
              {#if projects.length === 0}
                <h2>Seu primeiro projeto começa com uma URL</h2>
                <p>
                  Cole um repositório do GitHub. O YourGit baixa, detecta a
                  stack e monta um plano para você revisar antes de executar.
                </p>
                <button
                  class="button primary"
                  onclick={() => (addOpen = true)}
                >
                  <Icon name="plus" size={16} />
                  Adicionar projeto
                </button>
                <div class="empty-hints">
                  <span><Icon name="download" size={14} /> sem Git instalado</span>
                  <span><Icon name="shield" size={14} /> inspeção estática</span>
                  <span><Icon name="terminal" size={14} /> logs em tempo real</span>
                </div>
              {:else}
                <h2>Nenhum projeto neste filtro</h2>
                <p>Ajuste a busca ou escolha outro estado.</p>
                <button
                  class="button ghost"
                  onclick={() => {
                    filter = "all";
                    search = "";
                  }}>Limpar filtros</button
                >
              {/if}
            </section>
          {:else}
            <div class="project-grid">
              {#each visibleProjects as project (project.id)}
                <div
                  class="project-card"
                  role="button"
                  tabindex="0"
                  onclick={() => void openProject(project.id)}
                  onkeydown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      void openProject(project.id);
                    }
                  }}
                >
                  <div class="card-top">
                    <div class="project-avatar">
                      {project.name.slice(0, 1).toUpperCase()}
                    </div>
                    <div class="project-meta">
                      <h3>{project.name}</h3>
                      <p>
                        {project.owner ?? "local"}/{project.repo ??
                          project.name}
                      </p>
                    </div>
                    <StatusBadge state={project.state} />
                  </div>

                  <div class="card-tags">
                    <span class="tag"
                      ><Icon name="github" size={12} /> GitHub</span
                    >
                    {#if project.requestedRef}
                      <span class="tag mono">{project.requestedRef}</span>
                    {/if}
                    {#if project.depsReady}
                      <span class="tag ok">deps ok</span>
                    {:else}
                      <span class="tag">deps pendentes</span>
                    {/if}
                  </div>

                  <div class="card-footer">
                    <span class="muted small mono"
                      >{project.subdir === "." ? "raiz" : project.subdir}</span
                    >
                    <div class="card-actions">
                      {#if activeStates.includes(project.state)}
                        <button
                          class="icon-button danger"
                          title="Parar"
                          onclick={(event) => {
                            event.stopPropagation();
                            void stopById(project.id);
                          }}
                        >
                          <Icon name="stop" size={15} />
                        </button>
                      {:else}
                        <button
                          class="icon-button"
                          title="Executar"
                          onclick={(event) => {
                            event.stopPropagation();
                            void startById(project.id);
                          }}
                        >
                          <Icon name="play" size={15} />
                        </button>
                      {/if}
                      <button
                        class="icon-button"
                        title="Abrir pasta"
                        onclick={(event) => {
                          event.stopPropagation();
                          void api
                            .openProjectFolder(project.id)
                            .catch((folderError) => setError(folderError));
                        }}
                      >
                        <Icon name="folder" size={15} />
                      </button>
                      <Icon name="chevron-right" size={16} />
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      {:else if view === "catalog"}
        <section class="page-head">
          <div>
            <h2>Dois canais, dois níveis de confiança</h2>
            <p>
              Receitas oficiais revisadas pela equipe e receitas da comunidade
              claramente separadas. Nada executa sem você ver os comandos.
            </p>
          </div>
        </section>

        <div class="channel-tabs">
          <button
            class="channel-tab"
            class:active={catalogChannel === "official"}
            onclick={() => (catalogChannel = "official")}
          >
            <Icon name="shield" size={16} />
            Oficial
            <span class="tag ok">assinado</span>
          </button>
          <button
            class="channel-tab"
            class:active={catalogChannel === "community"}
            onclick={() => (catalogChannel = "community")}
          >
            <Icon name="users" size={16} />
            Comunidade
            <span class="tag warn">não verificado</span>
          </button>
        </div>

        {#if catalogChannel === "official"}
          <section class="channel-hero official">
            <div class="channel-icon"><Icon name="shield" size={28} /></div>
            <div>
              <span class="eyebrow">Canal oficial</span>
              <h3>Curadoria completa, commits fixados e catálogo assinado.</h3>
              <p>
                Cada receita passa por revisão de licença, comandos, tamanho e
                funcionamento. A atualização do índice usa assinatura Ed25519 e
                mantém um snapshot offline.
              </p>
            </div>
            <div class="channel-meta">
              <span>10–20 entradas planejadas</span>
              <span>commit SHA</span>
              <span>revisão humana</span>
            </div>
          </section>
        {:else}
          <section class="channel-hero community">
            <div class="channel-icon"><Icon name="users" size={28} /></div>
            <div>
              <span class="eyebrow">Canal comunidade</span>
              <h3>Aberto para receitas da comunidade, com aviso de risco.</h3>
              <p>
                Recebe lint automático, verificação de URL e commit, e nunca é
                misturado ao canal oficial. O selo “Comunidade” e a revisão do
                plano deixam o nível de confiança explícito.
              </p>
            </div>
            <div class="channel-meta">
              <span>envio por PR</span>
              <span>lint automático</span>
              <span>promoção para oficial</span>
            </div>
          </section>
        {/if}

        <section class="empty-state compact-empty">
          <div class="empty-icon"><Icon name="book" size={28} /></div>
          <h2>Catálogo em construção</h2>
          <p>
            A estrutura dos dois canais já está definida. A instalação pelo
            catálogo entra na fase 3; por enquanto, adicione repositórios pela
            URL.
          </p>
          <button class="button primary" onclick={() => (addOpen = true)}>
            <Icon name="plus" size={16} />
            Adicionar por URL
          </button>
        </section>
      {:else}
        <section class="page-head">
          <div>
            <h2>Configurações</h2>
            <p>Ambiente local, Docker e informações do aplicativo.</p>
          </div>
          <button
            class="button ghost"
            disabled={dockerRefreshing}
            onclick={() => void refreshDocker()}
          >
            <Icon name="refresh" size={15} />
            {dockerRefreshing ? "Verificando..." : "Verificar Docker"}
          </button>
        </section>

        <div class="settings-grid">
          <section class="panel setting-card">
            <div class="setting-icon"><Icon name="folder" size={20} /></div>
            <div>
              <h3>Workspace</h3>
              <p class="mono small path">{appInfo?.workspaceDir ?? "—"}</p>
              <p class="muted small">
                Onde os repositórios são baixados. Pode ser alterado pela
                variável YOURGIT_WORKSPACE.
              </p>
            </div>
          </section>

          <section class="panel setting-card">
            <div class="setting-icon"><Icon name="file" size={20} /></div>
            <div>
              <h3>Dados do app</h3>
              <p class="mono small path">{appInfo?.dataDir ?? "—"}</p>
              <p class="muted small">
                Banco SQLite, logs e backups ficam aqui.
              </p>
            </div>
          </section>

          <section class="panel setting-card">
            <div class="setting-icon">
              <Icon name="box" size={20} />
            </div>
            <div>
              <h3>Docker</h3>
              <p>
                {#if appInfo?.dockerAvailable}
                  <span class="tag ok">disponível</span>
                  <span class="mono small"
                    >{appInfo.dockerVersion ?? ""}</span
                  >
                {:else}
                  <span class="tag warn">indisponível</span>
                {/if}
              </p>
              <p class="muted small">
                O sandbox Docker será o padrão na fase 2. Hoje a execução é
                nativa.
              </p>
            </div>
          </section>

          <section class="panel setting-card">
            <div class="setting-icon"><Icon name="shield" size={20} /></div>
            <div>
              <h3>Segurança</h3>
              <p class="small">
                Downloads passam por validação de zip-slip, symlinks e limites.
                Logs têm tokens conhecidos redigidos.
              </p>
              <p class="muted small">
                Execute apenas repositórios confiáveis enquanto o isolamento
                não estiver disponível.
              </p>
            </div>
          </section>

          <section class="panel setting-card">
            <div class="setting-icon"><Icon name="layers" size={20} /></div>
            <div>
              <h3>Catálogo</h3>
              <p class="small">
                Canais oficial e comunidade definidos. O índice remoto e a
                assinatura entram na fase 3.
              </p>
            </div>
          </section>

          <section class="panel setting-card">
            <div class="setting-icon"><Icon name="info" size={20} /></div>
            <div>
              <h3>Sobre</h3>
              <p class="small">YourGit {appInfo?.version ?? "0.1.0"} · MVP</p>
              <p class="muted small">
                Feito com Tauri 2, Rust e Svelte 5. Windows primeiro, Linux
                depois.
              </p>
            </div>
          </section>
        </div>
      {/if}
    </div>
  </main>
</div>

{#if confirmRequest}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) resolveConfirm(false);
    }}
  >
    <div
      class="modal modal-confirm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-title"
      tabindex="-1"
    >
      <header class="modal-head">
        <div>
          <span class="eyebrow">Confirmação</span>
          <h2 id="confirm-title">{confirmRequest.title}</h2>
        </div>
      </header>
      <div class="modal-body">
        <p class="confirm-message">{confirmRequest.message}</p>
      </div>
      <footer class="modal-foot">
        <button class="button ghost" onclick={() => resolveConfirm(false)}>
          Cancelar
        </button>
        <button
          class="button"
          class:primary={!confirmRequest.danger}
          class:danger={confirmRequest.danger}
          onclick={() => resolveConfirm(true)}
        >
          {confirmRequest.confirmLabel}
        </button>
      </footer>
    </div>
  </div>
{/if}

{#if addOpen}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget && !adding) addOpen = false;
    }}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="add-project-title"
      tabindex="-1"
    >
      <header class="modal-head">
        <div>
          <span class="eyebrow">Novo projeto</span>
          <h2 id="add-project-title">Adicionar do GitHub</h2>
        </div>
        <button
          class="icon-button"
          aria-label="Fechar"
          disabled={adding}
          onclick={() => (addOpen = false)}
        >
          <Icon name="x" size={18} />
        </button>
      </header>

      <div class="modal-body">
        <label class="field">
          <span>URL do repositório</span>
          <div class="input-wrap">
            <Icon name="github" size={17} />
            <input
              bind:value={url}
              placeholder="https://github.com/proprietário/repositório"
              disabled={adding}
              spellcheck="false"
              autocapitalize="off"
              onkeydown={(event) => {
                if (event.key === "Enter") void addProject();
              }}
            />
          </div>
        </label>
        <p class="field-hint">
          Aceita a URL raiz, <code>/tree/&lt;branch&gt;/&lt;pasta&gt;</code> e
          <code>git@github.com:proprietário/repositório.git</code>.
        </p>

        <div class="callout info">
          <Icon name="shield" size={17} />
          <div>
            <strong>Nada é executado agora</strong>
            <p>
              O YourGit baixa o zip, valida os caminhos, extrai e detecta a
              stack. Você revisa o plano antes de qualquer comando rodar.
            </p>
          </div>
        </div>

        {#if addError}
          <div class="inline-error">
            <strong>{addError.title}</strong>
            <p>{addError.message}</p>
            {#if addError.hint}<span>{addError.hint}</span>{/if}
          </div>
        {/if}

        {#if adding}
          <div class="progress-line">
            <span class="spinner"></span>
            <span>{busyLabel || "Preparando projeto..."}</span>
          </div>
        {/if}
      </div>

      <footer class="modal-foot">
        <button
          class="button ghost"
          disabled={adding}
          onclick={() => (addOpen = false)}
        >
          Cancelar
        </button>
        <button
          class="button primary"
          disabled={adding || !url.trim()}
          onclick={() => void addProject()}
        >
          {#if adding}
            <span class="spinner small"></span>
            Preparando...
          {:else}
            <Icon name="download" size={16} />
            Baixar e inspecionar
          {/if}
        </button>
      </footer>
    </div>
  </div>
{/if}

{#if toast}
  <div class="toast" role="status">
    <Icon name="check" size={16} />
    <span>{toast}</span>
  </div>
{/if}
