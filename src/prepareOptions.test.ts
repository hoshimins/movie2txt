import { describe, expect, it } from "vitest";
import { buildPreparationOptions } from "./prepareOptions";

describe("buildPreparationOptions", () => {
  it("omits silence cut settings when the toggle is off", () => {
    expect(
      buildPreparationOptions({
        maxLineWidth: 23,
        downloadMode: "video",
        audioFormat: "m4a",
        silenceCutEnabled: false,
        silenceCutSettings: { noiseDb: -35, minSilenceMs: 400, paddingMs: 150 },
      }),
    ).toEqual({
      maxLineWidth: 23,
      downloadMode: "video",
      audioFormat: null,
      silenceCut: null,
    });
  });

  it("includes silence cut settings when the toggle is on", () => {
    expect(
      buildPreparationOptions({
        maxLineWidth: 23,
        downloadMode: "audio",
        audioFormat: "mp3",
        silenceCutEnabled: true,
        silenceCutSettings: { noiseDb: -34, minSilenceMs: 500, paddingMs: 120 },
      }),
    ).toEqual({
      maxLineWidth: 23,
      downloadMode: "audio",
      audioFormat: "mp3",
      silenceCut: { noiseDb: -34, minSilenceMs: 500, paddingMs: 120 },
    });
  });
});
