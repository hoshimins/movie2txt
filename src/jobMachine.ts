import { assign, setup } from "xstate";
import type { JobPhase, JobProgress, JobStatus } from "./types";

interface JobContext {
  jobId?: string;
  projectId?: string;
  phase: JobPhase;
  status: JobStatus;
  logs: string[];
  error?: string;
}

type JobEvent =
  | { type: "START"; jobId: string; projectId: string }
  | { type: "PROGRESS"; progress: JobProgress }
  | { type: "RESET" };

const initialContext: JobContext = {
  phase: "queued",
  status: "running",
  logs: [],
};

export const preparationJobMachine = setup({
  types: {} as {
    context: JobContext;
    events: JobEvent;
  },
  actions: {
    startJob: assign(({ event }) => {
      if (event.type !== "START") return {};
      return {
        jobId: event.jobId,
        projectId: event.projectId,
        phase: "queued",
        status: "running",
        error: undefined,
        logs: ["準備ジョブを開始しました"],
      };
    }),
    appendProgress: assign(({ context, event }) => {
      if (event.type !== "PROGRESS") return {};
      return {
        phase: event.progress.phase,
        status: event.progress.status,
        logs: [...context.logs, event.progress.message],
        error: event.progress.status === "failed" ? event.progress.message : context.error,
      };
    }),
    reset: assign(() => ({ ...initialContext })),
  },
  guards: {
    isSucceeded: ({ event }) =>
      event.type === "PROGRESS" && event.progress.status === "succeeded",
    isFailed: ({ event }) =>
      event.type === "PROGRESS" && event.progress.status === "failed",
    isCancelled: ({ event }) =>
      event.type === "PROGRESS" && event.progress.status === "cancelled",
  },
}).createMachine({
  id: "preparationJob",
  initial: "idle",
  context: () => ({ ...initialContext }),
  states: {
    idle: {
      on: {
        START: {
          target: "running",
          actions: "startJob",
        },
      },
    },
    running: {
      on: {
        PROGRESS: [
          {
            target: "succeeded",
            guard: "isSucceeded",
            actions: "appendProgress",
          },
          {
            target: "failed",
            guard: "isFailed",
            actions: "appendProgress",
          },
          {
            target: "cancelled",
            guard: "isCancelled",
            actions: "appendProgress",
          },
          {
            actions: "appendProgress",
          },
        ],
        RESET: {
          target: "idle",
          actions: "reset",
        },
      },
    },
    succeeded: {
      on: {
        RESET: {
          target: "idle",
          actions: "reset",
        },
        START: {
          target: "running",
          actions: "startJob",
        },
      },
    },
    failed: {
      on: {
        RESET: {
          target: "idle",
          actions: "reset",
        },
        START: {
          target: "running",
          actions: "startJob",
        },
      },
    },
    cancelled: {
      on: {
        RESET: {
          target: "idle",
          actions: "reset",
        },
        START: {
          target: "running",
          actions: "startJob",
        },
      },
    },
  },
});
