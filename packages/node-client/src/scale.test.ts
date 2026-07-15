import { performance } from "node:perf_hooks";

import { describe, expect, it } from "vitest";

import { toChecklistRecord } from "./checklist-converters";
import { toMessageRecord, toPeerRecord } from "./message-converters";
import { toEventProjectionRecord, toTelemetryPositionRecord } from "./projection-converters";

const LOCAL_OPERATION_BUDGET_MS = 500;
const SAMPLE_COUNT = 5;

function percentile95(samples: number[]): number {
  const ordered = [...samples].sort((left, right) => left - right);
  return ordered[Math.ceil(ordered.length * 0.95) - 1] ?? Number.POSITIVE_INFINITY;
}

function measure(operation: () => unknown): number[] {
  operation();
  return Array.from({ length: SAMPLE_COUNT }, () => {
    const startedAt = performance.now();
    operation();
    return performance.now() - startedAt;
  });
}

function expectWithinBudget(samples: number[]): void {
  expect(percentile95(samples)).toBeLessThan(LOCAL_OPERATION_BUDGET_MS);
}

describe("defined scale matrix", () => {
  it("projects 1,000 peers within the local operation budget", () => {
    const peers = Array.from({ length: 1_000 }, (_, index) => ({
      destinationHex: index.toString(16).padStart(32, "0"),
      displayName: `peer-${index}`,
      state: index % 2 === 0 ? "Connected" : "Disconnected",
      saved: index % 3 === 0,
      lastSeenAtMs: 1_700_000_000_000 + index,
    }));

    expectWithinBudget(measure(() => peers.map(toPeerRecord)));
  });

  it("projects 10,000 messages within the local operation budget", () => {
    const messages = Array.from({ length: 10_000 }, (_, index) => ({
      messageIdHex: index.toString(16).padStart(64, "0"),
      conversationId: `conversation-${index % 100}`,
      direction: index % 2 === 0 ? "Inbound" : "Outbound",
      destinationHex: (index % 1_000).toString(16).padStart(32, "0"),
      bodyUtf8: `message-${index}`,
      method: "Direct",
      state: index % 2 === 0 ? "Received" : "Delivered",
      transportState: "TransportDelivered",
      applicationAckState: "Accepted",
      updatedAtMs: 1_700_000_000_000 + index,
    }));

    expectWithinBudget(measure(() => messages.map(toMessageRecord)));
  });

  it("projects 1,000 events and telemetry records within budget", () => {
    const events = Array.from({ length: 1_000 }, (_, index) => ({
      commandId: `event-${index}`,
      commandType: "mission.registry.log_entry.upsert",
      sourceIdentity: index.toString(16).padStart(32, "0"),
      uid: `event-${index}`,
      missionUid: "scale-mission",
      content: `event body ${index}`,
      callsign: `unit-${index}`,
      updatedAt: 1_700_000_000_000 + index,
    }));
    const telemetry = events.map((_, index) => ({
      callsign: `unit-${index}`,
      lat: 44.6 + index / 100_000,
      lon: -63.6 - index / 100_000,
      updatedAt: 1_700_000_000_000 + index,
    }));

    expectWithinBudget(measure(() => events.map(toEventProjectionRecord)));
    expectWithinBudget(measure(() => telemetry.map(toTelemetryPositionRecord)));
  });

  it("projects 100 checklists with 200 tasks each within budget", () => {
    const checklists = Array.from({ length: 100 }, (_, checklistIndex) => ({
      uid: `checklist-${checklistIndex}`,
      name: `Checklist ${checklistIndex}`,
      description: "Scale acceptance checklist",
      expectedTaskCount: 200,
      tasks: Array.from({ length: 200 }, (_, taskIndex) => ({
        taskUid: `task-${checklistIndex}-${taskIndex}`,
        number: taskIndex + 1,
        userStatus: taskIndex % 4 === 0 ? "COMPLETE" : "PENDING",
        taskStatus: taskIndex % 4 === 0 ? "COMPLETE" : "PENDING",
        cells: [{
          cellUid: `cell-${checklistIndex}-${taskIndex}`,
          taskUid: `task-${checklistIndex}-${taskIndex}`,
          columnUid: "description",
          value: `Task ${taskIndex}`,
        }],
      })),
    }));

    expectWithinBudget(measure(() => checklists.map(toChecklistRecord)));
  });
});
