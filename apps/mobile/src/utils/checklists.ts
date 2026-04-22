export type ChecklistSegment = "live" | "templates";
export type ChecklistStatus = "active" | "late" | "completed";
export type ChecklistFilter = "all" | ChecklistStatus;
export type ChecklistTaskStatus = "pending" | "late" | "completed";
export type ChecklistTaskMetaTone = "clock" | "alert" | "done";

export interface ChecklistRecord {
  id: string;
  title: string;
  subtitle: string;
  status: ChecklistStatus;
  progress: number;
  statusCountLabel: string;
  scheduledAt: string;
  teamLabel: string;
  compatibilityLabel: string;
}

export interface ChecklistTask {
  id: string;
  title: string;
  description: string;
  status: ChecklistTaskStatus;
  metaLabel: string;
  metaTone: ChecklistTaskMetaTone;
}

export interface ChecklistDetail {
  id: string;
  heroTitle: string;
  heroSubtitle: string;
  progress: number;
  progressLabel: string;
  pendingLabel: string;
  tasksHeading: string;
  tasks: ChecklistTask[];
}

export const liveChecklists: ChecklistRecord[] = [
  {
    id: "recon-sector-07",
    title: "Reconnaissance Sector 07",
    subtitle: "Operational sweep",
    status: "active",
    progress: 15,
    statusCountLabel: "11 pending",
    scheduledAt: "May 5, 2026 08:04 PM",
    teamLabel: "Team Alpha",
    compatibilityLabel: "RCH compatible",
  },
  {
    id: "field-triage-setup",
    title: "Field Triage Setup",
    subtitle: "Medical staging",
    status: "late",
    progress: 72,
    statusCountLabel: "2 overdue",
    scheduledAt: "May 5, 2026 07:00 PM",
    teamLabel: "Med Team",
    compatibilityLabel: "RCH compatible",
  },
  {
    id: "relay-node-recovery",
    title: "Relay Node Recovery",
    subtitle: "Mesh infrastructure",
    status: "completed",
    progress: 100,
    statusCountLabel: "Synced",
    scheduledAt: "May 4, 2026 04:30 PM",
    teamLabel: "Comms Team",
    compatibilityLabel: "RCH compatible",
  },
];

export const templateChecklists: ChecklistRecord[] = [
  {
    id: "template-recon",
    title: "Recon Patrol Template",
    subtitle: "Reusable field template",
    status: "active",
    progress: 0,
    statusCountLabel: "Template ready",
    scheduledAt: "Template library",
    teamLabel: "Operations",
    compatibilityLabel: "RCH compatible",
  },
  {
    id: "template-medical",
    title: "Medical Staging Template",
    subtitle: "Rapid casualty setup",
    status: "active",
    progress: 0,
    statusCountLabel: "Template ready",
    scheduledAt: "Template library",
    teamLabel: "Medical",
    compatibilityLabel: "RCH compatible",
  },
];

const checklistDetails: Record<string, ChecklistDetail> = {
  "recon-sector-07": {
    id: "recon-sector-07",
    heroTitle: "Reconnaissance",
    heroSubtitle: "Operational Sector 07",
    progress: 15,
    progressLabel: "15% complete",
    pendingLabel: "11 pending",
    tasksHeading: "Active Tasks",
    tasks: [
      {
        id: "mission-briefing",
        title: "Mission Briefing",
        description: "Review objectives and extraction points with the tactical team leader.",
        status: "pending",
        metaLabel: "05/05/2026, 08:04 PM",
        metaTone: "clock",
      },
      {
        id: "equipment-check",
        title: "Equipment Check",
        description: "Verify radio kit, batteries, and medical loadout before departure.",
        status: "late",
        metaLabel: "05/05/2026, 07:00 PM",
        metaTone: "alert",
      },
      {
        id: "secure-communications",
        title: "Secure Communications",
        description: "Establish encrypted handshake with relay node and confirm route availability.",
        status: "completed",
        metaLabel: "Completed 2h ago",
        metaTone: "done",
      },
      {
        id: "bio-metric-sync",
        title: "Bio-Metric Sync",
        description: "Calibrate vital sensors to match environmental atmospheric pressure.",
        status: "pending",
        metaLabel: "05/06/2026, 04:00 AM",
        metaTone: "clock",
      },
    ],
  },
  "field-triage-setup": {
    id: "field-triage-setup",
    heroTitle: "Field Triage",
    heroSubtitle: "Medical Staging",
    progress: 72,
    progressLabel: "72% complete",
    pendingLabel: "2 overdue",
    tasksHeading: "Open Tasks",
    tasks: [
      {
        id: "erect-shelters",
        title: "Erect Shelter Grid",
        description: "Stage triage lanes and erect treatment shelter spacing for incoming casualties.",
        status: "completed",
        metaLabel: "Completed 35m ago",
        metaTone: "done",
      },
      {
        id: "stock-red-line",
        title: "Red Line Supplies",
        description: "Confirm trauma kits, IV stock, and airway support at the red treatment line.",
        status: "late",
        metaLabel: "05/05/2026, 07:00 PM",
        metaTone: "alert",
      },
      {
        id: "patient-tracking",
        title: "Patient Tracking",
        description: "Assign casualty tags and verify transfer routing to the evacuation channel.",
        status: "pending",
        metaLabel: "05/05/2026, 09:10 PM",
        metaTone: "clock",
      },
    ],
  },
  "relay-node-recovery": {
    id: "relay-node-recovery",
    heroTitle: "Relay Node",
    heroSubtitle: "Mesh Infrastructure",
    progress: 100,
    progressLabel: "100% complete",
    pendingLabel: "Synced",
    tasksHeading: "Completed Tasks",
    tasks: [
      {
        id: "power-cycle",
        title: "Power Cycle",
        description: "Bring the relay stack back online and verify stable power draw.",
        status: "completed",
        metaLabel: "Completed 4h ago",
        metaTone: "done",
      },
      {
        id: "route-audit",
        title: "Route Audit",
        description: "Confirm message propagation path and heartbeat recovery through the backbone.",
        status: "completed",
        metaLabel: "Completed 3h ago",
        metaTone: "done",
      },
      {
        id: "link-check",
        title: "Link Check",
        description: "Validate downstream peers received the restored route and sync windows.",
        status: "completed",
        metaLabel: "Completed 2h ago",
        metaTone: "done",
      },
    ],
  },
  "template-recon": {
    id: "template-recon",
    heroTitle: "Recon Patrol",
    heroSubtitle: "Reusable Field Template",
    progress: 0,
    progressLabel: "0% complete",
    pendingLabel: "Template ready",
    tasksHeading: "Template Tasks",
    tasks: [
      {
        id: "template-briefing",
        title: "Mission Briefing",
        description: "Reusable patrol briefing block with objectives, routes, and extraction criteria.",
        status: "pending",
        metaLabel: "Template step",
        metaTone: "clock",
      },
      {
        id: "template-comms",
        title: "Comms Setup",
        description: "Preset radio and relay validation step for pre-deployment verification.",
        status: "pending",
        metaLabel: "Template step",
        metaTone: "clock",
      },
    ],
  },
  "template-medical": {
    id: "template-medical",
    heroTitle: "Medical Staging",
    heroSubtitle: "Rapid Casualty Setup",
    progress: 0,
    progressLabel: "0% complete",
    pendingLabel: "Template ready",
    tasksHeading: "Template Tasks",
    tasks: [
      {
        id: "template-triage-grid",
        title: "Stage Triage Grid",
        description: "Reusable layout step for intake, treatment, and evacuation lanes.",
        status: "pending",
        metaLabel: "Template step",
        metaTone: "clock",
      },
      {
        id: "template-med-logistics",
        title: "Medical Logistics",
        description: "Default inventory and casualty tracking block for rapid deployment.",
        status: "pending",
        metaLabel: "Template step",
        metaTone: "clock",
      },
    ],
  },
};

const allChecklists = [...liveChecklists, ...templateChecklists];

export function getChecklistRecords(segment: ChecklistSegment): ChecklistRecord[] {
  return segment === "templates" ? templateChecklists : liveChecklists;
}

export function getChecklistRecordById(checklistId: string): ChecklistRecord | undefined {
  return allChecklists.find((record) => record.id === checklistId);
}

export function getChecklistDetailById(checklistId: string): ChecklistDetail | undefined {
  return checklistDetails[checklistId];
}
