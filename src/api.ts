import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
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

export function startPreparationJob(projectId: string, options: PreparationOptions) {
  return invoke<JobId>("start_preparation_job", { projectId, options });
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
