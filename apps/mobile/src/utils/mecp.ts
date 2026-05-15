export type MecpSeverity = 0 | 1 | 2 | 3;

export type MecpCategoryCode =
  | "M"
  | "T"
  | "W"
  | "S"
  | "P"
  | "C"
  | "R"
  | "D"
  | "L"
  | "X"
  | "H"
  | "B";

export interface MecpSeverityOption {
  value: MecpSeverity;
  label: string;
  meaning: string;
  status: "Red" | "Yellow" | "Green" | "Unknown";
}

export interface MecpCategoryOption {
  code: MecpCategoryCode;
  label: string;
  icon: "medical" | "terrain" | "weather" | "supplies" | "position" | "coordination" | "response" | "drill" | "leisure" | "threat" | "resources" | "beacon";
}

export interface MecpEventCode {
  code: string;
  label: string;
}

export interface ParsedMecpMessage {
  valid: boolean;
  severity: MecpSeverity | null;
  codes: string[];
  category: MecpCategoryCode | null;
  details: string;
  raw: string;
}

const MECP_PREFIX = "MECP/";
const CODE_PATTERN = /^[A-Z]\d{2}$/;
const VALID_SEVERITIES = new Set<number>([0, 1, 2, 3]);

export const MECP_SEVERITIES: MecpSeverityOption[] = [
  { value: 0, label: "Mayday", meaning: "Critical", status: "Red" },
  { value: 1, label: "Urgent", meaning: "Challenge", status: "Yellow" },
  { value: 2, label: "Safety", meaning: "OK", status: "Green" },
  { value: 3, label: "Routine", meaning: "Normal", status: "Unknown" },
];

export const MECP_CATEGORIES: MecpCategoryOption[] = [
  { code: "M", label: "Medical", icon: "medical" },
  { code: "T", label: "Terrain / Infrastructure", icon: "terrain" },
  { code: "W", label: "Weather / Environment", icon: "weather" },
  { code: "S", label: "Supplies", icon: "supplies" },
  { code: "P", label: "Position / Movement", icon: "position" },
  { code: "C", label: "Coordination", icon: "coordination" },
  { code: "R", label: "Response", icon: "response" },
  { code: "D", label: "Drill / Test", icon: "drill" },
  { code: "L", label: "Life / Leisure", icon: "leisure" },
  { code: "X", label: "Threat / Security", icon: "threat" },
  { code: "H", label: "Have / Offer Resources", icon: "resources" },
  { code: "B", label: "Beacon", icon: "beacon" },
];

export const MECP_EVENT_CODES: Record<MecpCategoryCode, MecpEventCode[]> = {
  M: [
    { code: "M01", label: "Injury" },
    { code: "M02", label: "Unconscious person" },
    { code: "M03", label: "Breathing difficulty" },
    { code: "M04", label: "Cardiac event" },
    { code: "M05", label: "Hypothermia" },
    { code: "M06", label: "Severe bleeding" },
    { code: "M07", label: "Fracture / immobile" },
    { code: "M08", label: "Burns" },
    { code: "M09", label: "Multiple casualties" },
    { code: "M10", label: "Deceased" },
    { code: "M11", label: "Animal bite / sting" },
    { code: "M12", label: "Allergic reaction / anaphylaxis" },
    { code: "M13", label: "Poisoning / toxic exposure" },
    { code: "M14", label: "Persons located alive" },
    { code: "M15", label: "Area searched, no victims found" },
  ],
  T: [
    { code: "T01", label: "Road blocked" },
    { code: "T02", label: "Bridge out" },
    { code: "T03", label: "Building collapsed" },
    { code: "T04", label: "Flooding" },
    { code: "T05", label: "Landslide" },
    { code: "T06", label: "Power out" },
    { code: "T07", label: "Fire" },
    { code: "T08", label: "Avalanche" },
    { code: "T09", label: "Path impassable" },
    { code: "T10", label: "Shelter available" },
    { code: "T11", label: "Drowning / water rescue needed" },
    { code: "T12", label: "Water contamination" },
    { code: "T13", label: "Earthquake" },
    { code: "T14", label: "Gas leak" },
    { code: "T15", label: "Chemical spill / HAZMAT" },
    { code: "T16", label: "Vehicle accident" },
    { code: "T17", label: "Vehicle fire" },
  ],
  W: [
    { code: "W01", label: "Storm approaching" },
    { code: "W02", label: "Visibility zero" },
    { code: "W03", label: "Extreme cold" },
    { code: "W04", label: "Extreme heat" },
    { code: "W05", label: "Air quality danger" },
    { code: "W06", label: "Tsunami / tidal surge warning" },
  ],
  S: [
    { code: "S01", label: "Need water" },
    { code: "S02", label: "Need food" },
    { code: "S03", label: "Need medication" },
    { code: "S04", label: "Need battery / power" },
    { code: "S05", label: "Need fuel" },
    { code: "S06", label: "Need tools / equipment" },
  ],
  P: [
    { code: "P01", label: "Stranded / stuck" },
    { code: "P02", label: "Evacuating toward" },
    { code: "P03", label: "Sheltering in place" },
    { code: "P04", label: "En route to" },
    { code: "P05", label: "At GPS coordinates" },
    { code: "P06", label: "Lost" },
    { code: "P07", label: "Group separated" },
  ],
  C: [
    { code: "C01", label: "Send rescue" },
    { code: "C02", label: "Need transport" },
    { code: "C03", label: "Relay this message" },
    { code: "C04", label: "Confirm received" },
    { code: "C05", label: "How many people" },
    { code: "C06", label: "What is status" },
    { code: "C07", label: "Can you reach" },
    { code: "C08", label: "Rendezvous at" },
  ],
  R: [
    { code: "R01", label: "Acknowledged" },
    { code: "R02", label: "Help coming" },
    { code: "R03", label: "ETA [minutes]" },
    { code: "R04", label: "Cannot assist" },
    { code: "R05", label: "Redirecting to" },
    { code: "R06", label: "Stand by" },
    { code: "R07", label: "Situation resolved / all clear" },
  ],
  D: [
    { code: "D01", label: "This is a drill" },
    { code: "D02", label: "This is a test" },
    { code: "D03", label: "End of drill" },
    { code: "D04", label: "Ignore previous - sent in error" },
  ],
  L: [
    { code: "L01", label: "Beer / drinks" },
    { code: "L02", label: "Coffee" },
    { code: "L03", label: "Food ready" },
    { code: "L04", label: "Summit reached" },
    { code: "L05", label: "At camp" },
    { code: "L06", label: "Running late" },
    { code: "L07", label: "Good signal here" },
    { code: "L08", label: "Photo opportunity" },
    { code: "L09", label: "Wildlife spotted" },
    { code: "L10", label: "Beautiful view" },
    { code: "L11", label: "Trail conditions good" },
    { code: "L12", label: "Trail conditions bad" },
    { code: "L13", label: "Need a break" },
    { code: "L14", label: "Heading home" },
    { code: "L15", label: "Good morning / check-in" },
    { code: "L16", label: "Good night" },
    { code: "L17", label: "Thank you" },
    { code: "L18", label: "Having fun" },
    { code: "L19", label: "Festival / event here" },
    { code: "L20", label: "Node test / ping" },
  ],
  X: [
    { code: "X01", label: "Dangerous person / threat nearby" },
    { code: "X02", label: "Area unsafe - avoid" },
    { code: "X03", label: "Gunfire / explosions heard" },
    { code: "X04", label: "Civil unrest / crowd danger" },
    { code: "X05", label: "Theft / looting reported" },
    { code: "X06", label: "Authorities / emergency services present" },
    { code: "X07", label: "Checkpoint / road closure" },
  ],
  H: [
    { code: "H01", label: "Have water available" },
    { code: "H02", label: "Have food available" },
    { code: "H03", label: "Have medical supplies" },
    { code: "H04", label: "Have power / charging" },
    { code: "H05", label: "Have fuel" },
    { code: "H06", label: "Have tools / equipment" },
    { code: "H07", label: "Have shelter / space for [N]pax" },
    { code: "H08", label: "Have transport / vehicle" },
  ],
  B: [
    { code: "B01", label: "Automated distress beacon active" },
    { code: "B02", label: "Beacon acknowledged" },
    { code: "B03", label: "Cancel beacon - I am OK" },
  ],
};

export function isMecpCategoryCode(value: string): value is MecpCategoryCode {
  return MECP_CATEGORIES.some((category) => category.code === value);
}

export function mecpSeverityLabel(value: MecpSeverity | null): string {
  return MECP_SEVERITIES.find((severity) => severity.value === value)?.label ?? "Unknown";
}

export function mecpCategoryLabel(value: MecpCategoryCode | null): string {
  return MECP_CATEGORIES.find((category) => category.code === value)?.label ?? "MECP";
}

export function mecpEventLabel(code: string): string {
  const category = code.charAt(0);
  if (!isMecpCategoryCode(category)) {
    return code;
  }
  const match = MECP_EVENT_CODES[category].find((event) => event.code === code);
  return match ? `${match.code} ${match.label}` : code;
}

export function encodeMecpMessage(input: {
  severity: MecpSeverity;
  code: string;
  details?: string;
}): string {
  const normalizedCode = input.code.trim().toUpperCase();
  const details = input.details?.trim();
  const base = `${MECP_PREFIX}${input.severity}/${normalizedCode}`;
  return details ? `${base} ${details}` : base;
}

export function parseMecpMessage(input: string): ParsedMecpMessage {
  const raw = input.trim();
  const invalid: ParsedMecpMessage = {
    valid: false,
    severity: null,
    codes: [],
    category: null,
    details: "",
    raw,
  };

  if (!raw.startsWith(MECP_PREFIX)) {
    return invalid;
  }

  const severity = Number.parseInt(raw.charAt(5), 10);
  if (!VALID_SEVERITIES.has(severity) || raw.charAt(6) !== "/") {
    return invalid;
  }

  const tokens = raw.slice(7).split(/\s+/).filter((token) => token.length > 0);
  const codes: string[] = [];
  let detailsStart = tokens.length;
  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index].toUpperCase();
    if (!CODE_PATTERN.test(token)) {
      detailsStart = index;
      break;
    }
    codes.push(token);
  }

  const categoryCandidate = codes[0]?.charAt(0) ?? "";
  const category: MecpCategoryCode | null = isMecpCategoryCode(categoryCandidate)
    ? categoryCandidate
    : null;
  if (codes.length === 0 || !category) {
    return invalid;
  }

  return {
    valid: true,
    severity: severity as MecpSeverity,
    codes,
    category,
    details: tokens.slice(detailsStart).join(" "),
    raw,
  };
}
