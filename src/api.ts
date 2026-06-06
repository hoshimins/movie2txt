import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  AudioMergePreview,
  AudioMergeTarget,
  AudioSplitPreview,
  ClipMarker,
  DownloadSource,
  HighlightRequestBundle,
  HighlightRequestOptions,
  JobId,
  PreparationOptions,
  Project,
  ProjectSnapshot,
  ProjectSummary,
  SilenceAnalysis,
  SilenceCutResult,
  SilenceCutSettings,
  SubtitleDocument,
  SubtitleEntry,
} from "./types";

export function listProjects() {
  return invoke<ProjectSummary[]>("list_projects");
}

export function createProject(name: string, source: DownloadSource) {
  return invoke<Project>("create_project", { name, source });
}

export function openProject(projectId: string) {
  return invoke<ProjectSnapshot>("open_project", { projectId });
}

export function attachExistingMedia(projectId: string, path: string) {
  return invoke<Project>("attach_existing_media", { projectId, path });
}

export function startPreparationJob(projectId: string, options: PreparationOptions) {
  return invoke<JobId>("start_preparation_job", { projectId, options });
}

export function startMediaDownloadJob(projectId: string) {
  return invoke<JobId>("start_media_download_job", { projectId });
}

export function previewAudioSplit(projectId: string, splitMinutes: number) {
  return invoke<AudioSplitPreview>("preview_audio_split", { projectId, splitMinutes });
}

export function startAudioSplitJob(projectId: string, splitMinutes: number) {
  return invoke<JobId>("start_audio_split_job", { projectId, splitMinutes });
}

export function scanAudioMerge(sourceDir: string) {
  return invoke<AudioMergePreview>("scan_audio_merge", { sourceDir });
}

export function startAudioMergeJob(projectId: string, target: AudioMergeTarget, sourceDir: string) {
  return invoke<JobId>("start_audio_merge_job", { projectId, target, sourceDir });
}

export function startVocalsTranscriptionJob(projectId: string, maxLineWidth?: number | null) {
  return invoke<JobId>("start_vocals_transcription_job", { projectId, maxLineWidth });
}

export function cancelJob(jobId: string) {
  return invoke<void>("cancel_job", { jobId });
}

export function readSubtitles(projectId: string) {
  return invoke<SubtitleDocument>("read_subtitles", { projectId });
}

export function saveSubtitles(projectId: string, entries: SubtitleEntry[]) {
  return invoke<Project>("save_subtitles", { projectId, entries });
}

export function saveClipMarkers(projectId: string, markers: ClipMarker[]) {
  return invoke<Project>("save_clip_markers", { projectId, markers });
}

export function detectSilence(projectId: string, settings: SilenceCutSettings) {
  return invoke<SilenceAnalysis>("detect_silence", { projectId, settings });
}

export function createSilenceCut(projectId: string, settings: SilenceCutSettings) {
  return invoke<SilenceCutResult>("create_silence_cut", { projectId, settings });
}

export function generateHighlightRequest(projectId: string, options: HighlightRequestOptions) {
  return invoke<HighlightRequestBundle>("generate_highlight_request", { projectId, options });
}

export function importHighlightCandidates(projectId: string, filePath: string) {
  return invoke<ClipMarker[]>("import_highlight_candidates", { projectId, filePath });
}

export function getAppSettings() {
  return invoke<AppSettings>("get_app_settings");
}

export function updateAppSettings(settings: AppSettings) {
  return invoke<void>("update_app_settings", { settings });
}

export function updateYtdlp() {
  return invoke<void>("update_ytdlp");
}

export function openPath(path: string) {
  return invoke<void>("open_path", { path });
}
