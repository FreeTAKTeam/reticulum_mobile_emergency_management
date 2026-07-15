import type { ChecklistColumnRecord, ChecklistRecord, ChecklistTaskRecord, ChecklistTemplateRecord, ReticulumNodeClient } from "./contracts";

export type ChecklistCreateInput = Parameters<ReticulumNodeClient["createChecklistFromTemplate"]>[0];
export type ChecklistUpdateInput = Parameters<ReticulumNodeClient["updateChecklist"]>[0];
export type ChecklistStatusInput = Parameters<ReticulumNodeClient["setChecklistTaskStatus"]>[0];
export type ChecklistRowAddInput = Parameters<ReticulumNodeClient["addChecklistTaskRow"]>[0];
export type ChecklistRowDeleteInput = Parameters<ReticulumNodeClient["deleteChecklistTaskRow"]>[0];
export type ChecklistRowStyleInput = Parameters<ReticulumNodeClient["setChecklistTaskRowStyle"]>[0];
export type ChecklistCellInput = Parameters<ReticulumNodeClient["setChecklistTaskCell"]>[0];
export type ChecklistTemplateCsvInput = Parameters<ReticulumNodeClient["importChecklistTemplateCsv"]>[0];

export function cloneChecklistRecord(record: ChecklistRecord): ChecklistRecord {
  return JSON.parse(JSON.stringify(record)) as ChecklistRecord;
}

export function cloneChecklistTemplateRecord(record: ChecklistTemplateRecord): ChecklistTemplateRecord {
  return JSON.parse(JSON.stringify(record)) as ChecklistTemplateRecord;
}

export function defaultChecklistColumns(): ChecklistColumnRecord[] {
  return [
    {
      columnUid: "col-due-relative-dtg",
      columnName: "CompletedDTG",
      displayOrder: 0,
      columnType: "RELATIVE_TIME",
      columnEditable: false,
      isRemovable: false,
      systemKey: "DUE_RELATIVE_DTG",
    },
    {
      columnUid: "col-task",
      columnName: "Task",
      displayOrder: 1,
      columnType: "SHORT_STRING",
      columnEditable: true,
      isRemovable: false,
      systemKey: "task",
    },
    {
      columnUid: "col-description",
      columnName: "Detail",
      displayOrder: 2,
      columnType: "LONG_STRING",
      columnEditable: true,
      isRemovable: true,
    },
    {
      columnUid: "col-owner",
      columnName: "Owner",
      displayOrder: 3,
      columnType: "SHORT_STRING",
      columnEditable: true,
      isRemovable: true,
    },
  ];
}

export function defaultChecklistTask(taskUid: string, number: number, title: string, detail: string): ChecklistTaskRecord {
  const now = new Date().toISOString();
  return {
    taskUid,
    number,
    userStatus: "PENDING",
    taskStatus: "PENDING",
    isLate: false,
    updatedAt: now,
    dueRelativeMinutes: number * 30,
    legacyValue: title,
    lineBreakEnabled: false,
    cells: [
      {
        cellUid: `${taskUid}:col-task`,
        taskUid,
        columnUid: "col-task",
        value: title,
        updatedAt: now,
      },
      {
        cellUid: `${taskUid}:col-description`,
        taskUid,
        columnUid: "col-description",
        value: detail,
        updatedAt: now,
      },
      {
        cellUid: `${taskUid}:col-owner`,
        taskUid,
        columnUid: "col-owner",
        value: "Unassigned",
        updatedAt: now,
      },
    ],
  };
}

export function parseCsvRows(csvText: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  for (let index = 0; index < csvText.length; index += 1) {
    const char = csvText[index];
    const next = csvText[index + 1];
    if (quoted) {
      if (char === "\"" && next === "\"") {
        cell += "\"";
        index += 1;
      } else if (char === "\"") {
        quoted = false;
      } else {
        cell += char;
      }
      continue;
    }
    if (char === "\"") {
      quoted = true;
    } else if (char === ",") {
      row.push(cell.replace(/^\uFEFF/, "").trim());
      cell = "";
    } else if (char === "\n") {
      row.push(cell.replace(/^\uFEFF/, "").trim());
      rows.push(row);
      row = [];
      cell = "";
    } else if (char !== "\r") {
      cell += char;
    }
  }
  row.push(cell.replace(/^\uFEFF/, "").trim());
  rows.push(row);
  return rows.filter((entry) => entry.some((value) => value.trim().length > 0));
}

export function normalizeCsvHeader(value: string): string {
  return value.replace(/^\uFEFF/, "").toLowerCase().replace(/[^a-z0-9]/g, "");
}

export function isDueCsvHeader(value: string): boolean {
  return ["completeddtg", "due", "duerelativedtg", "duerelativeminutes", "dueminutes"].includes(normalizeCsvHeader(value));
}

export function isTitleCsvHeader(value: string): boolean {
  return ["item", "task", "name", "title"].includes(normalizeCsvHeader(value));
}

export function isDescriptionCsvHeader(value: string): boolean {
  return ["description", "detail", "details", "notes"].includes(normalizeCsvHeader(value));
}

export function csvColumnUid(header: string, index: number, used: Map<string, number>): string {
  const slug = header
    .replace(/^\uFEFF/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "") || `column-${index + 1}`;
  const base = `col-${slug}`;
  const count = (used.get(base) ?? 0) + 1;
  used.set(base, count);
  return count === 1 ? base : `${base}-${count}`;
}

export function parseDueRelativeMinutes(value: string): number {
  let text = value.trim().toLowerCase();
  if (!text || text.startsWith("-")) {
    throw new Error("Invalid CompletedDTG value");
  }
  if (text.startsWith("+")) {
    text = text.slice(1).trim();
  }
  const hhmm = text.match(/^(\d+):(\d{1,2})$/);
  if (hhmm) {
    const hours = Number(hhmm[1]);
    const minutes = Number(hhmm[2]);
    if (!Number.isFinite(hours) || !Number.isFinite(minutes) || minutes >= 60) {
      throw new Error("Invalid CompletedDTG value");
    }
    return hours * 60 + minutes;
  }
  const hours = text.match(/^(\d+)\s*(h|hour|hours)$/);
  if (hours) {
    return Number(hours[1]) * 60;
  }
  const minutes = Number(text);
  if (!Number.isInteger(minutes) || minutes < 0) {
    throw new Error("Invalid CompletedDTG value");
  }
  return minutes;
}

export function createInMemoryChecklistTemplateFromCsv(input: ChecklistTemplateCsvInput): ChecklistTemplateRecord {
  const name = input.name.trim();
  const rows = parseCsvRows(input.csvText);
  if (!name || rows.length < 2) {
    throw new Error("CSV must include a header row and at least one task row");
  }
  const headerRow = rows[0];
  const taskRows = rows.slice(1);
  const maxColumns = taskRows.reduce((max, row) => Math.max(max, row.length), headerRow.length);
  if (maxColumns === 0) {
    throw new Error("CSV header row is empty");
  }
  const headers = Array.from({ length: maxColumns }, (_, index) => headerRow[index]?.trim() || `Column ${index + 1}`);
  const dueHeaderIndex = headers.findIndex(isDueCsvHeader);
  const now = new Date().toISOString();
  const columns: ChecklistColumnRecord[] = [{
    columnUid: "col-due-relative-dtg",
    columnName: dueHeaderIndex >= 0 ? headers[dueHeaderIndex] : "CompletedDTG",
    displayOrder: 0,
    columnType: "RELATIVE_TIME",
    columnEditable: false,
    isRemovable: false,
    systemKey: "DUE_RELATIVE_DTG",
  }];
  const used = new Map<string, number>([["col-due-relative-dtg", 1]]);
  const headerColumnUids = new Map<number, string>();
  for (const [index, header] of headers.entries()) {
    if (index === dueHeaderIndex) {
      continue;
    }
    const columnUid = csvColumnUid(header, index, used);
    headerColumnUids.set(index, columnUid);
    columns.push({
      columnUid,
      columnName: header,
      displayOrder: columns.length,
      columnType: "SHORT_STRING",
      columnEditable: true,
      isRemovable: true,
    });
  }
  if (headerColumnUids.size === 0) {
    throw new Error("CSV must include at least one task data column");
  }
  const titleHeaderIndex = headers.findIndex((header, index) => index !== dueHeaderIndex && isTitleCsvHeader(header));
  const descriptionHeaderIndex = headers.findIndex((header, index) => index !== dueHeaderIndex && isDescriptionCsvHeader(header));
  const templateUid = input.templateUid?.trim() || `tmpl-web-${Date.now().toString(36)}`;
  const tasks = taskRows.map((row, index): ChecklistTaskRecord => {
    const number = index + 1;
    const taskUid = `${templateUid}-task-${number}`;
    const dueValue = dueHeaderIndex >= 0 ? row[dueHeaderIndex]?.trim() || "" : "";
    const dueRelativeMinutes = dueValue ? parseDueRelativeMinutes(dueValue) : number * 30;
    const title = (titleHeaderIndex >= 0 ? row[titleHeaderIndex]?.trim() : "")
      || headers.map((_, headerIndex) => headerIndex === dueHeaderIndex ? "" : row[headerIndex]?.trim() || "").find(Boolean)
      || `Task ${number}`;
    const notes = descriptionHeaderIndex >= 0 ? row[descriptionHeaderIndex]?.trim() || undefined : undefined;
    return {
      taskUid,
      number,
      userStatus: "PENDING",
      taskStatus: "PENDING",
      isLate: false,
      updatedAt: now,
      dueRelativeMinutes,
      notes,
      legacyValue: title,
      lineBreakEnabled: false,
      cells: [...headerColumnUids.entries()].map(([headerIndex, columnUid]) => ({
        cellUid: `${taskUid}:${columnUid}`,
        taskUid,
        columnUid,
        value: row[headerIndex]?.trim() || "",
        updatedAt: now,
      })),
    };
  });
  return {
    uid: templateUid,
    name,
    description: input.description?.trim() || "Imported CSV checklist template",
    version: 1,
    originType: "CSV_IMPORT",
    createdAt: now,
    updatedAt: now,
    sourceFilename: input.sourceFilename,
    columns,
    tasks,
  };
}

export function createDefaultChecklistTemplates(): ChecklistTemplateRecord[] {
  const now = new Date().toISOString();
  return [
    {
      uid: "tmpl-web-autonomous-emergency",
      name: "Autonomous Emergency Checklist",
      description: "Browser visual debugging template",
      version: 1,
      originType: "RCH_TEMPLATE",
      createdAt: now,
      updatedAt: now,
      columns: defaultChecklistColumns(),
      tasks: [
        defaultChecklistTask("tmpl-web-task-1", 1, "Confirm team readiness", "Verify operator, comms, and battery status."),
        defaultChecklistTask("tmpl-web-task-2", 2, "Prepare evacuation route", "Confirm the primary route and one alternate."),
        defaultChecklistTask("tmpl-web-task-3", 3, "Share situation update", "Broadcast current status to collaborating REM nodes."),
      ],
    },
  ];
}

