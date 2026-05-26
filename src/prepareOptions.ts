import type { DownloadMode, PreparationOptions, SilenceCutSettings } from "./types";

export function buildPreparationOptions(input: {
  maxLineWidth: number;
  downloadMode: DownloadMode;
  audioFormat: string;
  silenceCutEnabled: boolean;
  silenceCutSettings: SilenceCutSettings;
}): PreparationOptions {
  return {
    maxLineWidth: input.maxLineWidth,
    downloadMode: input.downloadMode,
    audioFormat: input.downloadMode === "audio" ? input.audioFormat : null,
    silenceCut: input.silenceCutEnabled ? input.silenceCutSettings : null,
  };
}
