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
