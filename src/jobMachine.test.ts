import { createActor } from "xstate";
import { describe, expect, it } from "vitest";
import { preparationJobMachine } from "./jobMachine";

describe("preparationJobMachine", () => {
  it("moves from idle to running and records progress", () => {
    const actor = createActor(preparationJobMachine);
    actor.start();

    actor.send({ type: "START", jobId: "job-1", projectId: "project-1" });
    actor.send({
      type: "PROGRESS",
      progress: {
        jobId: "job-1",
        projectId: "project-1",
        phase: "download",
        status: "running",
        message: "downloading",
      },
    });

    const snapshot = actor.getSnapshot();
    expect(snapshot.matches("running")).toBe(true);
    expect(snapshot.context.logs).toContain("downloading");
  });

  it("moves to succeeded when the backend reports completion", () => {
    const actor = createActor(preparationJobMachine);
    actor.start();

    actor.send({ type: "START", jobId: "job-1", projectId: "project-1" });
    actor.send({
      type: "PROGRESS",
      progress: {
        jobId: "job-1",
        projectId: "project-1",
        phase: "completed",
        status: "succeeded",
        message: "done",
      },
    });

    expect(actor.getSnapshot().matches("succeeded")).toBe(true);
  });
});
