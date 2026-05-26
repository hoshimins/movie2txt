import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Link, Outlet, RouterProvider, createRootRoute, createRoute, createRouter, useNavigate, useParams } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMachine } from "@xstate/react";
import { useEffect, useState } from "react";
import { z } from "zod";
import {
  cancelJob,
  createProject,
  getAppSettings,
  listProjects,
  openProject,
  startPreparationJob,
  updateAppSettings,
  updateYtdlp,
} from "./api";
import { preparationJobMachine } from "./jobMachine";
import { buildPreparationOptions } from "./prepareOptions";
import SubtitleEditor from "./SubtitleEditor";
import type {
  AppSettings,
  ClipMarker,
  DownloadMode,
  DownloadSource,
  JobProgress,
  PreparationOptions,
  ProjectSnapshot,
} from "./types";

const urlSchema = z.string().url("URLの形式で入力してください");

function AppShell() {
  return (
    <div className="app-frame">
      <header className="topbar">
        <Link to="/" className="brand">
          Movie2Text
        </Link>
        <nav>
          <Link to="/" activeProps={{ className: "active" }}>
            Projects
          </Link>
          <Link to="/settings" activeProps={{ className: "active" }}>
            Settings
          </Link>
        </nav>
      </header>
      <Outlet />
    </div>
  );
}

function ProjectListPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [sourceMode, setSourceMode] = useState<"url" | "local">("url");
  const [name, setName] = useState("");
  const [url, setUrl] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [formError, setFormError] = useState("");

  const projectsQuery = useQuery({
    queryKey: ["projects"],
    queryFn: listProjects,
  });

  const createMutation = useMutation({
    mutationFn: (input: { name: string; source: DownloadSource }) =>
      createProject(input.name, input.source),
    onSuccess: async (project) => {
      await queryClient.invalidateQueries({ queryKey: ["projects"] });
      await navigate({ to: "/projects/$projectId", params: { projectId: project.id } });
    },
  });

  const handlePickLocalFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Video",
          extensions: ["mp4", "avi", "mov", "mkv", "flv", "wmv", "m4a", "mp3", "wav"],
        },
      ],
    });

    if (typeof selected === "string") {
      setLocalPath(selected);
      if (!name) {
        setName(fileName(selected));
      }
    }
  };

  const handleCreate = () => {
    setFormError("");
    const projectName = name.trim() || (sourceMode === "url" ? "URL準備プロジェクト" : fileName(localPath));
    let source: DownloadSource;

    if (sourceMode === "url") {
      const parsed = urlSchema.safeParse(url.trim());
      if (!parsed.success) {
        setFormError(parsed.error.issues[0]?.message ?? "URLを確認してください");
        return;
      }
      source = { kind: "url", url: parsed.data };
    } else {
      if (!localPath) {
        setFormError("ローカル素材を選択してください");
        return;
      }
      source = { kind: "localFile", path: localPath };
    }

    createMutation.mutate({ name: projectName, source });
  };

  return (
    <main className="workspace">
      <section className="panel create-project">
        <div className="section-heading">
          <div>
            <p className="eyebrow">Prepare</p>
            <h1>切り抜き準備プロジェクト</h1>
          </div>
          <div className="segmented">
            <button className={sourceMode === "url" ? "active" : ""} onClick={() => setSourceMode("url")}>
              URL
            </button>
            <button className={sourceMode === "local" ? "active" : ""} onClick={() => setSourceMode("local")}>
              Local
            </button>
          </div>
        </div>

        <label>
          プロジェクト名
          <input value={name} onChange={(event) => setName(event.target.value)} placeholder="例: 配信切り抜き準備" />
        </label>

        {sourceMode === "url" ? (
          <label>
            動画URL
            <input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://..." />
          </label>
        ) : (
          <div className="file-picker-row">
            <button onClick={handlePickLocalFile}>素材を選択</button>
            <span title={localPath}>{localPath || "動画または音声ファイルを選択してください"}</span>
          </div>
        )}

        {formError && <p className="error-text">{formError}</p>}
        {createMutation.error && <p className="error-text">{String(createMutation.error)}</p>}
        <button className="primary" onClick={handleCreate} disabled={createMutation.isPending}>
          {createMutation.isPending ? "作成中..." : "プロジェクトを作成"}
        </button>
      </section>

      <section className="project-grid">
        {(projectsQuery.data ?? []).map((project) => (
          <Link key={project.id} to="/projects/$projectId" params={{ projectId: project.id }} className="project-card">
            <span>{sourceLabel(project.source)}</span>
            <strong>{project.name}</strong>
            <small>{project.mediaFileName ?? "素材未準備"} / {project.subtitleReady ? "字幕あり" : "字幕なし"} / {project.markerCount} markers</small>
          </Link>
        ))}
        {projectsQuery.data?.length === 0 && <div className="empty-state">まだプロジェクトがありません。</div>}
      </section>
    </main>
  );
}

function ProjectPage() {
  const { projectId } = useParams({ from: "/projects/$projectId" });
  const queryClient = useQueryClient();
  const [state, send] = useMachine(preparationJobMachine);
  const [downloadMode, setDownloadMode] = useState<DownloadMode>("video");
  const [audioFormat, setAudioFormat] = useState("m4a");
  const [silenceCutEnabled, setSilenceCutEnabled] = useState(false);
  const [silenceNoiseDb, setSilenceNoiseDb] = useState(-35);
  const [silenceDurationMs, setSilenceDurationMs] = useState(400);
  const [silencePaddingMs, setSilencePaddingMs] = useState(150);
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: getAppSettings });
  const projectQuery = useQuery({
    queryKey: ["project", projectId],
    queryFn: () => openProject(projectId),
  });

  useEffect(() => {
    const unlistenProgress = listen<JobProgress>("job-progress", (event) => {
      if (event.payload.projectId === projectId) {
        send({ type: "PROGRESS", progress: event.payload });
      }
    });
    const unlistenProject = listen<string>("project-updated", (event) => {
      if (event.payload === projectId) {
        void queryClient.invalidateQueries({ queryKey: ["project", projectId] });
        void queryClient.invalidateQueries({ queryKey: ["projects"] });
      }
    });

    return () => {
      void unlistenProgress.then((dispose) => dispose());
      void unlistenProject.then((dispose) => dispose());
    };
  }, [projectId, queryClient, send]);

  const startMutation = useMutation({
    mutationFn: (options: PreparationOptions) => startPreparationJob(projectId, options),
    onSuccess: (job) => send({ type: "START", jobId: job.id, projectId }),
  });

  const cancelMutation = useMutation({
    mutationFn: (jobId: string) => cancelJob(jobId),
  });

  const updateYtdlpMutation = useMutation({ mutationFn: updateYtdlp });

  const snapshot = projectQuery.data;
  const maxLineWidth = settingsQuery.data?.maxLineWidth ?? 23;
  const jobContext = state.context;
  const isRunning = state.matches("running");

  const startJob = () => {
    startMutation.mutate(buildPreparationOptions({
      maxLineWidth,
      downloadMode,
      audioFormat,
      silenceCutEnabled,
      silenceCutSettings: {
        noiseDb: silenceNoiseDb,
        minSilenceMs: silenceDurationMs,
        paddingMs: silencePaddingMs,
      },
    }));
  };

  if (projectQuery.isLoading) {
    return <main className="workspace"><div className="panel">読み込み中...</div></main>;
  }

  if (!snapshot) {
    return <main className="workspace"><div className="panel error-text">プロジェクトを読み込めませんでした。</div></main>;
  }

  return (
    <main className="project-workspace">
      <aside className="project-sidebar">
        <section className="panel">
          <p className="eyebrow">Project</p>
          <h1>{snapshot.project.name}</h1>
          <dl className="metadata-list">
            <div><dt>Source</dt><dd>{sourceLabel(snapshot.project.source)}</dd></div>
            <div><dt>Media</dt><dd>{snapshot.project.mediaAsset?.fileName ?? "未準備"}</dd></div>
            <div><dt>SRT</dt><dd>{snapshot.project.subtitlePath ? fileName(snapshot.project.subtitlePath) : "未生成"}</dd></div>
            <div><dt>Cut</dt><dd>{snapshot.project.silenceCutPath ? fileName(snapshot.project.silenceCutPath) : "未生成"}</dd></div>
          </dl>
        </section>

        <section className="panel">
          <h2>準備ジョブ</h2>
          {snapshot.project.source.kind === "url" && (
            <>
              <div className="segmented stretch">
                <button className={downloadMode === "video" ? "active" : ""} onClick={() => setDownloadMode("video")}>動画</button>
                <button className={downloadMode === "audio" ? "active" : ""} onClick={() => setDownloadMode("audio")}>音声</button>
              </div>
              {downloadMode === "audio" && (
                <select value={audioFormat} onChange={(event) => setAudioFormat(event.target.value)}>
                  <option value="m4a">m4a</option>
                  <option value="mp3">mp3</option>
                  <option value="wav">wav</option>
                </select>
              )}
            </>
          )}
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={silenceCutEnabled}
              onChange={(event) => setSilenceCutEnabled(event.target.checked)}
            />
            無音カット済み動画も作成
          </label>
          {silenceCutEnabled && (
            <div className="silence-settings">
              <label>
                無音しきい値 dB
                <input
                  type="number"
                  value={silenceNoiseDb}
                  onChange={(event) => setSilenceNoiseDb(Number(event.target.value))}
                />
              </label>
              <label>
                最小無音 ms
                <input
                  type="number"
                  min={100}
                  step={50}
                  value={silenceDurationMs}
                  onChange={(event) => setSilenceDurationMs(Number(event.target.value))}
                />
              </label>
              <label>
                余白 ms
                <input
                  type="number"
                  min={0}
                  step={50}
                  value={silencePaddingMs}
                  onChange={(event) => setSilencePaddingMs(Number(event.target.value))}
                />
              </label>
            </div>
          )}
          <button className="primary w-full" onClick={startJob} disabled={isRunning || startMutation.isPending}>
            {isRunning ? "実行中..." : silenceCutEnabled ? "字幕生成と無音カットを実行" : "URL取得から字幕生成まで実行"}
          </button>
          {isRunning && jobContext.jobId && (
            <button className="w-full" onClick={() => cancelMutation.mutate(jobContext.jobId!)}>キャンセル</button>
          )}
          <button className="w-full" onClick={() => updateYtdlpMutation.mutate()} disabled={updateYtdlpMutation.isPending}>
            yt-dlp更新
          </button>
        </section>

        <section className="panel log-panel">
          <div className="section-heading compact">
            <h2>Logs</h2>
            <span>{jobContext.phase}</span>
          </div>
          <textarea readOnly value={jobContext.logs.join("\n")} placeholder="ジョブログがここに表示されます。" />
          {jobContext.error && <p className="error-text">{jobContext.error}</p>}
        </section>
      </aside>

      <section className="editor-shell">
        {snapshot.project.subtitlePath && snapshot.project.mediaAsset ? (
          <SubtitleEditor
            projectId={snapshot.project.id}
            srtFilePath={snapshot.project.subtitlePath}
            videoFilePath={snapshot.project.mediaAsset.path}
            initialMarkers={snapshot.project.markers}
            onClose={() => undefined}
            onSave={() => void queryClient.invalidateQueries({ queryKey: ["project", projectId] })}
          />
        ) : (
          <ClipMarkerDraft snapshot={snapshot} />
        )}
      </section>
    </main>
  );
}

function ClipMarkerDraft({ snapshot }: { snapshot: ProjectSnapshot }) {
  return (
    <div className="panel empty-state">
      <h2>字幕生成待ち</h2>
      <p>{snapshot.project.source.kind === "url" ? "準備ジョブを実行すると、ダウンロードした素材をそのまま字幕生成に渡します。" : "ローカル素材から字幕生成を開始してください。"}</p>
    </div>
  );
}

function SettingsPage() {
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: getAppSettings });
  const [draft, setDraft] = useState<AppSettings>({
    maxLineWidth: 23,
  });

  useEffect(() => {
    if (settingsQuery.data) {
      setDraft(settingsQuery.data);
    }
  }, [settingsQuery.data]);

  const mutation = useMutation({
    mutationFn: updateAppSettings,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["settings"] }),
  });

  const update = (key: keyof AppSettings, value: string | number) => {
    setDraft((current) => ({ ...current, [key]: value === "" ? null : value }));
  };

  return (
    <main className="workspace narrow">
      <section className="panel settings-form">
        <p className="eyebrow">Settings</p>
        <h1>外部ツールと字幕設定</h1>
        <SettingsInput label="yt-dlp.exe" value={draft.ytdlpPath ?? ""} onChange={(value) => update("ytdlpPath", value)} />
        <SettingsInput label="ffmpeg.exe" value={draft.ffmpegPath ?? ""} onChange={(value) => update("ffmpegPath", value)} />
        <SettingsInput label="faster-whisper.exe" value={draft.whisperPath ?? ""} onChange={(value) => update("whisperPath", value)} />
        <label>
          1字幕あたりの最大文字数
          <input type="number" min={0} value={draft.maxLineWidth} onChange={(event) => update("maxLineWidth", Number(event.target.value))} />
        </label>
        <button className="primary" onClick={() => mutation.mutate(draft)} disabled={mutation.isPending}>
          {mutation.isPending ? "保存中..." : "設定を保存"}
        </button>
      </section>
    </main>
  );
}

function SettingsInput({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label>
      {label}
      <input value={value} onChange={(event) => onChange(event.target.value)} placeholder="未設定なら bundled / PATH / .env を順に利用" />
    </label>
  );
}

function sourceLabel(source: DownloadSource) {
  return source.kind === "url" ? source.url : fileName(source.path);
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

export function replaceMarkers(markers: ClipMarker[], next: ClipMarker) {
  return markers.some((marker) => marker.id === next.id)
    ? markers.map((marker) => (marker.id === next.id ? next : marker))
    : [...markers, next];
}

const rootRoute = createRootRoute({ component: AppShell });
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: ProjectListPage,
});
const projectRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/projects/$projectId",
  component: ProjectPage,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsPage,
});

const router = createRouter({
  routeTree: rootRoute.addChildren([indexRoute, projectRoute, settingsRoute]),
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

export default function App() {
  return <RouterProvider router={router} />;
}
