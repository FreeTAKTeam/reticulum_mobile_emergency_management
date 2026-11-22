export interface EventProjectionRecord {
  command_id: string;
  source: {
    rns_identity: string;
    display_name?: string;
  };
  timestamp: string;
  command_type: string;
  args: {
    entry_uid: string;
    mission_uid: string;
    content: string;
    callsign: string;
    server_time?: string;
    client_time?: string;
    keywords: string[];
    content_hashes: string[];
    source_identity?: string;
    source_display_name?: string;
    latitude?: number;
    longitude?: number;
  };
  correlation_id?: string;
  topics: string[];
  deleted_at?: number;
  updatedAt: number;
}
