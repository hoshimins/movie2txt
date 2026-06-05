export type DownloadSource =
  | { kind: "url"; url: string }
  | { kind: "localFile"; path: string };

export type DownloadMode = "video" | "audio";

export interface MediaAsset {
  path: string;
  fileName: string;
  source: DownloadSource;
}

export interface SubtitleEntry {
  index: number;
  start_time: string;
  end_time: string;
  text: string;
}

export interface ClipMarker {
  id: string;
  startMs: number;
  endMs: number;
  title: string;
  memo: string;
  tags: string[];
  sourceSubtitleIds: number[];
}

export interface SubtitleDocument {
  path?: string | null;
  entries: SubtitleEntry[];
}

export interface Project {
  id: string;
  name: string;
  source: DownloadSource;
  projectDir: string;
  mediaAsset?: MediaAsset | null;
  wavPath?: string | null;
  subtitlePath?: string | null;
  silenceCutPath?: string | null;
  timelineMapPath?: string | null;
  silenceCutSrtPath?: string | null;
  highlightRequestPath?: string | null;
  highlightCandidatesPath?: string | null;
  audioSplitDir?: string | null;
  vocalsSourceDir?: string | null;
  bgmSourceDir?: string | null;
  vocalsMergedPath?: string | null;
  bgmMergedPath?: string | null;
  markers: ClipMarker[];
  createdAt: number;
  updatedAt: number;
}

export interface ProjectSummary {
  id: string;
  name: string;
  source: DownloadSource;
  mediaFileName?: string | null;
  subtitleReady: boolean;
  markerCount: number;
  updatedAt: number;
}

export interface ProjectSnapshot {
  project: Project;
  subtitles: SubtitleDocument;
}

export interface PreparationOptions {
  maxLineWidth?: number | null;
  downloadMode: DownloadMode;
  audioFormat?: string | null;
  silenceCut?: SilenceCutSettings | null;
}

export interface SilenceCutSettings {
  noiseDb: number;
  minSilenceMs: number;
  paddingMs: number;
}

export interface SilenceSegment {
  startMs: number;
  endMs: number;
}

export interface KeepSegment {
  originalStartMs: number;
  originalEndMs: number;
  cutStartMs: number;
  cutEndMs: number;
}

export interface SilenceAnalysis {
  mediaPath: string;
  durationMs: number;
  settings: SilenceCutSettings;
  silenceSegments: SilenceSegment[];
  keepSegments: KeepSegment[];
  removedMs: number;
}

export interface SilenceCutResult {
  outputPath: string;
  timelineMapPath: string;
  subtitlePath: string;
  analysis: SilenceAnalysis;
}

export interface HighlightRequestOptions {
  maxCandidates: number;
}

export interface HighlightRequestBundle {
  requestPath: string;
  schemaPath: string;
  subtitlePath: string;
  candidateOutputPath: string;
}

export interface AudioSplitPreview {
  inputPath: string;
  outputDir: string;
  durationMs: number;
  splitMinutes: number;
  partCount: number;
  outputExists: boolean;
}

export interface AudioMergePreview {
  sourceDir: string;
  files: string[];
}

export type AudioMergeTarget = "vocals" | "bgm";

export interface AppSettings {
  ytdlpPath?: string | null;
  ffmpegPath?: string | null;
  whisperPath?: string | null;
  outputDir?: string | null;
  tmpDir?: string | null;
  maxLineWidth: number;
}

export interface JobId {
  id: string;
}

export type JobPhase =
  | "queued"
  | "download"
  | "extract-audio"
  | "transcribe"
  | "postprocess"
  | "detect-silence"
  | "cut-silence"
  | "split-audio"
  | "merge-audio"
  | "generate-highlight-request"
  | "completed"
  | "failed"
  | "cancelled";

export type JobStatus = "running" | "succeeded" | "failed" | "cancelled";

export interface JobProgress {
  jobId: string;
  projectId: string;
  phase: JobPhase;
  status: JobStatus;
  message: string;
}
