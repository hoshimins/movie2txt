import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { Link, Outlet, RouterProvider, createRootRoute, createRoute, createRouter, useNavigate, useParams } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMachine } from "@xstate/react";
import { useEffect, useState } from "react";
import { z } from "zod";
import {
  attachExistingMedia,
  cancelJob,
  createProject,
  getAppSettings,
  openPath,
  listProjects,
  openProject,
  previewAudioSplit,
  scanAudioMerge,
  startAudioMergeJob,
  startAudioSplitJob,
  startMediaDownloadJob,
  startPreparationJob,
  startVocalsTranscriptionJob,
  updateAppSettings,
  updateYtdlp,
} from "./api";
import { preparationJobMachine } from "./jobMachine";
import { buildPreparationOptions } from "./prepareOptions";
import SubtitleEditor from "./SubtitleEditor";
import type {
  AppSettings,
  AudioMergePreview,
  AudioMergeTarget,
  AudioSplitPreview,
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
  const [activeProjectTab, setActiveProjectTab] = useState<"audio" | "subtitles">("audio");
  const settingsQuery = useQuery({ queryKey: ["settings"], queryFn: getAppSettings });
  const projectQuery = useQuery({
    queryKey: ["project", projectId],
    queryFn: () => openProject(projectId),
  });
  const maxLineWidth = settingsQuery.data?.maxLineWidth ?? 23;

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

  const mediaDownloadMutation = useMutation({
    mutationFn: () => startMediaDownloadJob(projectId),
    onSuccess: (job) => send({ type: "START", jobId: job.id, projectId }),
  });

  const attachExistingMediaMutation = useMutation({
    mutationFn: (path: string) => attachExistingMedia(projectId, path),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["project", projectId] });
      await queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
  });

  const audioSplitMutation = useMutation({
    mutationFn: (splitMinutes: number) => startAudioSplitJob(projectId, splitMinutes),
    onSuccess: (job) => send({ type: "START", jobId: job.id, projectId }),
  });

  const audioMergeMutation = useMutation({
    mutationFn: (input: { target: AudioMergeTarget; sourceDir: string }) =>
      startAudioMergeJob(projectId, input.target, input.sourceDir),
    onSuccess: (job) => send({ type: "START", jobId: job.id, projectId }),
  });

  const vocalsTranscriptionMutation = useMutation({
    mutationFn: () => startVocalsTranscriptionJob(projectId, maxLineWidth),
    onSuccess: (job) => send({ type: "START", jobId: job.id, projectId }),
  });

  const cancelMutation = useMutation({
    mutationFn: (jobId: string) => cancelJob(jobId),
  });

  const updateYtdlpMutation = useMutation({ mutationFn: updateYtdlp });

  const snapshot = projectQuery.data;
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

      <section className="editor-shell project-main-panel">
        <div className="project-tabs">
          <button className={activeProjectTab === "audio" ? "active" : ""} onClick={() => setActiveProjectTab("audio")}>
            Audio Workflow
          </button>
          <button className={activeProjectTab === "subtitles" ? "active" : ""} onClick={() => setActiveProjectTab("subtitles")}>
            Subtitle Editor
          </button>
        </div>

        {activeProjectTab === "audio" ? (
          <AudioWorkflow
            snapshot={snapshot}
            projectId={projectId}
            isRunning={isRunning}
            splitPending={audioSplitMutation.isPending}
            mergePending={audioMergeMutation.isPending}
            downloadPending={mediaDownloadMutation.isPending}
            attachPending={attachExistingMediaMutation.isPending}
            attachError={attachExistingMediaMutation.error}
            transcriptionPending={vocalsTranscriptionMutation.isPending}
            onDownloadMedia={() => mediaDownloadMutation.mutate()}
            onAttachExistingMedia={(path) => attachExistingMediaMutation.mutate(path)}
            onSplitAudio={(splitMinutes) => audioSplitMutation.mutate(splitMinutes)}
            onMergeAudio={(target, sourceDir) => audioMergeMutation.mutate({ target, sourceDir })}
            onTranscribeVocals={() => vocalsTranscriptionMutation.mutate()}
          />
        ) : snapshot.project.subtitlePath && projectMediaPath(snapshot) ? (
          <SubtitleEditor
            projectId={snapshot.project.id}
            srtFilePath={snapshot.project.subtitlePath}
            videoFilePath={projectMediaPath(snapshot)!}
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

function AudioWorkflow({
  snapshot,
  projectId,
  isRunning,
  splitPending,
  mergePending,
  downloadPending,
  attachPending,
  attachError,
  transcriptionPending,
  onDownloadMedia,
  onAttachExistingMedia,
  onSplitAudio,
  onMergeAudio,
  onTranscribeVocals,
}: {
  snapshot: ProjectSnapshot;
  projectId: string;
  isRunning: boolean;
  splitPending: boolean;
  mergePending: boolean;
  downloadPending: boolean;
  attachPending: boolean;
  attachError: unknown;
  transcriptionPending: boolean;
  onDownloadMedia: () => void;
  onAttachExistingMedia: (path: string) => void;
  onSplitAudio: (splitMinutes: number) => void;
  onMergeAudio: (target: AudioMergeTarget, sourceDir: string) => void;
  onTranscribeVocals: () => void;
}) {
  const [splitMinutes, setSplitMinutes] = useState(60);
  const [vocalsSourceDir, setVocalsSourceDir] = useState(snapshot.project.vocalsSourceDir ?? "");
  const [bgmSourceDir, setBgmSourceDir] = useState(snapshot.project.bgmSourceDir ?? "");
  const mediaPath = projectMediaPath(snapshot);
  const canSplit = Boolean(mediaPath);
  const isBusy = isRunning || splitPending || mergePending || downloadPending || attachPending || transcriptionPending;

  useEffect(() => {
    setVocalsSourceDir(snapshot.project.vocalsSourceDir ?? "");
    setBgmSourceDir(snapshot.project.bgmSourceDir ?? "");
  }, [snapshot.project.vocalsSourceDir, snapshot.project.bgmSourceDir]);

  const splitPreviewQuery = useQuery<AudioSplitPreview>({
    queryKey: ["audio-split-preview", projectId, splitMinutes, mediaPath],
    queryFn: () => previewAudioSplit(projectId, splitMinutes),
    enabled: canSplit && splitMinutes > 0,
  });

  const vocalsPreviewQuery = useQuery<AudioMergePreview>({
    queryKey: ["audio-merge-preview", "vocals", vocalsSourceDir],
    queryFn: () => scanAudioMerge(vocalsSourceDir),
    enabled: Boolean(vocalsSourceDir),
  });

  const bgmPreviewQuery = useQuery<AudioMergePreview>({
    queryKey: ["audio-merge-preview", "bgm", bgmSourceDir],
    queryFn: () => scanAudioMerge(bgmSourceDir),
    enabled: Boolean(bgmSourceDir),
  });

  const pickMergeFolder = async (target: AudioMergeTarget) => {
    const selected = await open({
      directory: true,
      multiple: false,
    });
    if (typeof selected === "string") {
      if (target === "vocals") {
        setVocalsSourceDir(selected);
      } else {
        setBgmSourceDir(selected);
      }
    }
  };

  const pickExistingMedia = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "Video / Audio",
          extensions: ["mp4", "avi", "mov", "mkv", "flv", "wmv", "m4a", "mp3", "wav"],
        },
      ],
    });

    if (typeof selected === "string") {
      onAttachExistingMedia(selected);
    }
  };

  const splitPreview = splitPreviewQuery.data;
  const splitBlocked = !canSplit || splitMinutes < 1 || Boolean(splitPreview?.outputExists) || splitPreviewQuery.isError;

  return (
    <div className="audio-workflow">
      <section className="panel workflow-panel">
        <div className="section-heading compact">
          <div>
            <p className="eyebrow">Audio Workflow</p>
            <h2>動画取得と24-bit WAV分割</h2>
          </div>
          {snapshot.project.source.kind === "url" && (
            <div className="workflow-actions">
              {!snapshot.project.mediaAsset && (
                <button className="primary" onClick={onDownloadMedia} disabled={isBusy}>
                  {downloadPending ? "取得開始中..." : "URL動画を取得"}
                </button>
              )}
              <button onClick={pickExistingMedia} disabled={isBusy}>
                {attachPending ? "紐づけ中..." : "既存ファイルを選択"}
              </button>
            </div>
          )}
        </div>

        <dl className="metadata-list workflow-metadata">
          <div><dt>Input</dt><dd title={mediaPath ?? ""}>{mediaPath ? fileName(mediaPath) : "動画取得後に分割できます"}</dd></div>
          <div><dt>Format</dt><dd>24-bit WAV / 元サンプルレート・チャンネル維持</dd></div>
        </dl>
        {Boolean(attachError) && <p className="error-text">{String(attachError)}</p>}

        <div className="workflow-grid">
          <label>
            分割長（分）
            <input
              type="number"
              min={1}
              step={1}
              value={splitMinutes}
              onChange={(event) => setSplitMinutes(Math.max(1, Number(event.target.value) || 1))}
            />
          </label>
          <button
            className="primary"
            onClick={() => onSplitAudio(splitMinutes)}
            disabled={isBusy || splitBlocked}
          >
            {splitPending ? "分割開始中..." : "音声分割を実行"}
          </button>
        </div>

        {splitPreviewQuery.isLoading && canSplit && <p className="field-hint">分割プレビューを取得しています...</p>}
        {splitPreviewQuery.error && <p className="error-text">{String(splitPreviewQuery.error)}</p>}
        {splitPreview && (
          <div className={splitPreview.outputExists ? "workflow-preview warning-preview" : "workflow-preview"}>
            <PathRow label="出力先" path={splitPreview.outputDir} canOpen={splitPreview.outputExists} />
            <div><span>動画尺</span><strong>{formatDuration(splitPreview.durationMs)}</strong></div>
            <div><span>推定パート数</span><strong>{splitPreview.partCount} 件</strong></div>
            <div><span>状態</span><strong>{splitPreview.outputExists ? "既に存在します" : "実行できます"}</strong></div>
          </div>
        )}
      </section>

      <section className="workflow-columns">
        <MergePanel
          title="ボーカルWAVを結合"
          target="vocals"
          sourceDir={vocalsSourceDir}
          preview={vocalsPreviewQuery.data}
          error={vocalsPreviewQuery.error}
          isLoading={vocalsPreviewQuery.isLoading}
          isBusy={isBusy}
          pending={mergePending}
          outputPath={snapshot.project.vocalsMergedPath}
          onPickFolder={() => pickMergeFolder("vocals")}
          onMerge={() => vocalsSourceDir && onMergeAudio("vocals", vocalsSourceDir)}
        />
        <MergePanel
          title="BGM WAVを結合"
          target="bgm"
          sourceDir={bgmSourceDir}
          preview={bgmPreviewQuery.data}
          error={bgmPreviewQuery.error}
          isLoading={bgmPreviewQuery.isLoading}
          isBusy={isBusy}
          pending={mergePending}
          outputPath={snapshot.project.bgmMergedPath}
          onPickFolder={() => pickMergeFolder("bgm")}
          onMerge={() => bgmSourceDir && onMergeAudio("bgm", bgmSourceDir)}
        />
      </section>

      <section className="panel workflow-panel">
        <div className="section-heading compact">
          <div>
            <p className="eyebrow">Transcribe</p>
            <h2>結合ボーカルから字幕生成</h2>
          </div>
          <button
            className="primary"
            onClick={onTranscribeVocals}
            disabled={isBusy || !snapshot.project.vocalsMergedPath}
          >
            {transcriptionPending ? "字幕生成開始中..." : "結合ボーカルから字幕生成"}
          </button>
        </div>
        <PathRow label="入力WAV" path={snapshot.project.vocalsMergedPath} />
        <PathRow label="メイン字幕" path={snapshot.project.subtitlePath} />
      </section>

      <section className="panel workflow-panel">
        <div className="section-heading compact">
          <div>
            <p className="eyebrow">Artifacts</p>
            <h2>生成物一覧</h2>
          </div>
        </div>
        <div className="artifact-list">
          <PathRow label="分割フォルダ" path={snapshot.project.audioSplitDir} />
          <PathRow label="ボーカル入力フォルダ" path={snapshot.project.vocalsSourceDir} />
          <PathRow label="BGM入力フォルダ" path={snapshot.project.bgmSourceDir} />
          <PathRow label="ボーカル結合WAV" path={snapshot.project.vocalsMergedPath} />
          <PathRow label="BGM結合WAV" path={snapshot.project.bgmMergedPath} />
          <PathRow label="メイン字幕" path={snapshot.project.subtitlePath} />
        </div>
      </section>
    </div>
  );
}

function MergePanel({
  title,
  target,
  sourceDir,
  preview,
  error,
  isLoading,
  isBusy,
  pending,
  outputPath,
  onPickFolder,
  onMerge,
}: {
  title: string;
  target: AudioMergeTarget;
  sourceDir: string;
  preview?: AudioMergePreview;
  error: unknown;
  isLoading: boolean;
  isBusy: boolean;
  pending: boolean;
  outputPath?: string | null;
  onPickFolder: () => void;
  onMerge: () => void;
}) {
  const canMerge = Boolean(preview?.files.length);
  return (
    <section className="panel workflow-panel">
      <div className="section-heading compact">
        <div>
          <p className="eyebrow">{target === "vocals" ? "Vocals" : "BGM"}</p>
          <h2>{title}</h2>
        </div>
        <button onClick={onPickFolder} disabled={isBusy}>フォルダ選択</button>
      </div>
      <PathRow label="入力フォルダ" path={sourceDir} />
      <PathRow label="出力WAV" path={outputPath} />
      {isLoading && <p className="field-hint">WAV一覧を読み込んでいます...</p>}
      {Boolean(error) && <p className="error-text">{String(error)}</p>}
      {preview && (
        <div className="wav-preview">
          <div className="wav-preview-header">
            <span>{preview.files.length} files</span>
            <button className="primary" onClick={onMerge} disabled={isBusy || !canMerge}>
              {pending ? "結合開始中..." : "結合実行"}
            </button>
          </div>
          <ol>
            {preview.files.slice(0, 12).map((path) => (
              <li key={path} title={path}>{fileName(path)}</li>
            ))}
          </ol>
          {preview.files.length > 12 && <p className="field-hint">他 {preview.files.length - 12} 件</p>}
        </div>
      )}
    </section>
  );
}

function PathRow({ label, path, canOpen = true }: { label: string; path?: string | null; canOpen?: boolean }) {
  return (
    <div className="path-row">
      <span>{label}</span>
      <strong title={path ?? ""}>{path || "未生成"}</strong>
      {path && canOpen && <button onClick={() => openPath(folderLikePath(path) ? path : directoryPath(path))}>開く</button>}
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

function projectMediaPath(snapshot: ProjectSnapshot) {
  return snapshot.project.mediaAsset?.path ?? (snapshot.project.source.kind === "localFile" ? snapshot.project.source.path : null);
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

function directoryPath(path: string) {
  const normalized = path.replace(/\\/g, "/");
  const index = normalized.lastIndexOf("/");
  return index > 0 ? path.slice(0, index) : path;
}

function formatDuration(durationMs: number) {
  const totalSeconds = Math.round(durationMs / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

function folderLikePath(path: string) {
  return !/\.[^\\/]+$/.test(path);
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
