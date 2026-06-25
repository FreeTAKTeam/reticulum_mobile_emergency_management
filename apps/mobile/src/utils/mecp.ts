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

export interface MecpCoordinates {
  latitude: number;
  longitude: number;
}

export interface MecpDecodedExtras {
  callsign: string | null;
  etaMinutes: number | null;
  language: string | null;
  pax: number | null;
  references: string[];
  coordinates: MecpCoordinates | null;
  timestamp: string | null;
}

export interface DecodedMecpCode {
  code: string;
  category: MecpCategoryCode;
  label: string;
  known: boolean;
}

export interface DecodedMecpMessage extends ParsedMecpMessage {
  byteLength: number;
  codeDetails: DecodedMecpCode[];
  extras: MecpDecodedExtras;
  warnings: string[];
}

export interface MecpEncodeExtras {
  callsign?: string;
  coordinates?: MecpCoordinates;
  etaMinutes?: number;
  language?: string;
  pax?: number;
  references?: string[];
  timestamp?: string;
}

const MECP_PREFIX = "MECP/";
const CODE_PATTERN = /^[A-Z]\d{2}$/;
const CALLSIGN_PATTERN = /^~([A-Za-z0-9][A-Za-z0-9_-]*)$/;
const COORDINATE_PATTERN = /^(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)$/;
const ETA_PATTERN = /^(\d{1,4})(?:m|min)?$/i;
const LANGUAGE_PATTERN = /^@([A-Za-z]{2,3})$/;
const PAX_PATTERN = /^(\d{1,4})pax$/i;
const REFERENCE_PATTERN = /^#([A-Za-z0-9][A-Za-z0-9_.-]*)$/;
const TIMESTAMP_PATTERN = /^@([01]\d|2[0-3])([0-5]\d)$/;
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

function createEmptyExtras(): MecpDecodedExtras {
  return {
    callsign: null,
    etaMinutes: null,
    language: null,
    pax: null,
    references: [],
    coordinates: null,
    timestamp: null,
  };
}

function createInvalidDecodedMecpMessage(raw: string, warnings: string[] = []): DecodedMecpMessage {
  return {
    valid: false,
    severity: null,
    codes: [],
    category: null,
    details: "",
    raw,
    byteLength: new TextEncoder().encode(raw).length,
    codeDetails: [],
    extras: createEmptyExtras(),
    warnings,
  };
}

function findEventCode(code: string): MecpEventCode | undefined {
  const category = code.charAt(0);
  if (!isMecpCategoryCode(category)) {
    return undefined;
  }
  return MECP_EVENT_CODES[category].find((event) => event.code === code);
}

function normalizeCodeList(input: { code?: string; codes?: string[] }): string[] {
  const rawCodes = input.codes ?? (input.code ? [input.code] : []);
  return rawCodes
    .map((code) => code.trim().toUpperCase())
    .filter((code) => code.length > 0);
}

function formatCoordinate(value: number): string {
  return Number.isInteger(value) ? value.toFixed(0) : Number.parseFloat(value.toFixed(6)).toString();
}

function normalizeReference(reference: string): string {
  const trimmed = reference.trim();
  if (!trimmed) {
    return "";
  }
  return trimmed.startsWith("#") ? trimmed : `#${trimmed}`;
}

export function encodeMecpMessage(input: {
  severity: MecpSeverity;
  code?: string;
  codes?: string[];
  details?: string;
  extras?: MecpEncodeExtras;
  mode?: "rem" | "portable";
}): string {
  const normalizedCodes = normalizeCodeList(input);
  const details = input.details?.trim();
  const tokens = normalizedCodes.filter((code) => CODE_PATTERN.test(code));
  const extras = input.extras;
  if (extras?.pax !== undefined && Number.isFinite(extras.pax) && extras.pax > 0) {
    tokens.push(`${Math.floor(extras.pax)}pax`);
  }
  if (extras?.coordinates) {
    tokens.push(`${formatCoordinate(extras.coordinates.latitude)},${formatCoordinate(extras.coordinates.longitude)}`);
  }
  for (const reference of extras?.references ?? []) {
    const normalized = normalizeReference(reference);
    if (normalized) {
      tokens.push(normalized);
    }
  }
  if (extras?.etaMinutes !== undefined && Number.isFinite(extras.etaMinutes) && extras.etaMinutes >= 0) {
    tokens.push(`${Math.floor(extras.etaMinutes)}`);
  }
  if (extras?.language) {
    const language = extras.language.trim().replace(/^@/, "");
    if (language) {
      tokens.push(`@${language.toLowerCase()}`);
    }
  }
  if (input.mode === "portable") {
    const timestamp = extras?.timestamp?.trim().replace(/^@/, "");
    const callsign = extras?.callsign?.trim().replace(/^~/, "");
    if (timestamp) {
      tokens.push(`@${timestamp}`);
    }
    if (callsign) {
      tokens.push(`~${callsign}`);
    }
  }
  if (details) {
    tokens.push(details);
  }
  return `${MECP_PREFIX}${input.severity}/${tokens.join(" ")}`;
}

export function decodeMecpMessage(input: string): DecodedMecpMessage {
  const raw = input.trim();

  if (!raw.startsWith(MECP_PREFIX)) {
    return createInvalidDecodedMecpMessage(raw);
  }

  const severity = Number.parseInt(raw.charAt(5), 10);
  if (!VALID_SEVERITIES.has(severity) || raw.charAt(6) !== "/") {
    return createInvalidDecodedMecpMessage(raw, ["Invalid MECP severity or separator."]);
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

  if (codes.length === 0) {
    return createInvalidDecodedMecpMessage(raw, ["MECP message does not contain an event code."]);
  }

  const codeDetails: DecodedMecpCode[] = [];
  const warnings: string[] = [];
  for (const code of codes) {
    const categoryCandidate = code.charAt(0);
    if (!isMecpCategoryCode(categoryCandidate)) {
      return createInvalidDecodedMecpMessage(raw, [`Invalid MECP category "${categoryCandidate}".`]);
    }
    const eventCode = findEventCode(code);
    if (!eventCode) {
      warnings.push(`Unknown MECP event code "${code}".`);
    }
    codeDetails.push({
      code,
      category: categoryCandidate,
      label: eventCode ? eventCode.label : code,
      known: Boolean(eventCode),
    });
  }

  const extras = createEmptyExtras();
  let etaConsumed = false;
  for (const token of tokens.slice(detailsStart)) {
    const timestampMatch = token.match(TIMESTAMP_PATTERN);
    if (timestampMatch) {
      extras.timestamp = `${timestampMatch[1]}${timestampMatch[2]}`;
      continue;
    }
    const languageMatch = token.match(LANGUAGE_PATTERN);
    if (languageMatch) {
      extras.language = languageMatch[1].toLowerCase();
      continue;
    }
    const callsignMatch = token.match(CALLSIGN_PATTERN);
    if (callsignMatch) {
      extras.callsign = callsignMatch[1];
      continue;
    }
    const paxMatch = token.match(PAX_PATTERN);
    if (paxMatch) {
      extras.pax = Number.parseInt(paxMatch[1], 10);
      continue;
    }
    const coordinateMatch = token.match(COORDINATE_PATTERN);
    if (coordinateMatch) {
      const latitude = Number.parseFloat(coordinateMatch[1]);
      const longitude = Number.parseFloat(coordinateMatch[2]);
      if (latitude >= -90 && latitude <= 90 && longitude >= -180 && longitude <= 180) {
        extras.coordinates = { latitude, longitude };
      } else {
        warnings.push(`Coordinates outside valid range: "${token}".`);
      }
      continue;
    }
    const referenceMatch = token.match(REFERENCE_PATTERN);
    if (referenceMatch) {
      extras.references.push(`#${referenceMatch[1]}`);
      continue;
    }
    const etaMatch = token.match(ETA_PATTERN);
    if (!etaConsumed && codes.includes("R03") && etaMatch) {
      extras.etaMinutes = Number.parseInt(etaMatch[1], 10);
      etaConsumed = true;
    }
  }

  return {
    valid: true,
    severity: severity as MecpSeverity,
    codes,
    category: codeDetails[0].category,
    details: tokens.slice(detailsStart).join(" "),
    raw,
    byteLength: new TextEncoder().encode(raw).length,
    codeDetails,
    extras,
    warnings,
  };
}

export function parseMecpMessage(input: string): ParsedMecpMessage {
  return decodeMecpMessage(input);
}
