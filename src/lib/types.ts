/** Shapes returned by the Rust backend (`invoke('snapshot')`). */

export interface GpuInfo {
  available: boolean;
  name: string;
  util_pct: number | null;
  mem_used_mib: number | null;
  mem_total_mib: number | null;
  temp_c: number | null;
  power_w: number | null;
  clock_mhz: number | null;
  throttle: string;
  error: string | null;
  collected_ms: number;
}

export interface ModelInfo {
  served_name: string;
  checkpoint: string;
  checkpoint_exists: boolean;
  endpoint: string;
  config_path: string;
  chat_template: string;
  dtype: string;
  kv_cache_dtype: string;
  max_model_len: number | null;
  gpu_memory_utilization: number | null;
  mtp_depth: number | null;
  max_num_seqs: number | null;
  prefix_caching: boolean;
}

export type ServerState = "ready" | "loading" | "stopped" | "error";

export interface ServerInfo {
  state: string;
  listening: boolean;
  pid: number | null;
  process_name: string | null;
  uptime_s: number | null;
  health_ok: boolean;
  latency_ms: number | null;
  models: string[];
  error: string | null;
  endpoint: string;
}

export interface BlackoutInfo {
  engine_active: boolean;
  display_held: boolean;
  stopped: boolean;
  reason: string;
  updated_utc: string;
  idle_seconds: number;
  follow_server: boolean;
  cover: string;
  enabled: boolean;
  stop_file_present: boolean;
  state_path: string;
  phase: string;
}

export interface Snapshot {
  seq: number;
  collected_ms: number;
  collect_duration_ms: number;
  host: string;
  model: ModelInfo;
  server: ServerInfo;
  gpu: GpuInfo;
  blackout: BlackoutInfo;
  notes: string[];
}

export interface ReadyReport {
  startup_ms: number;
  arch: string;
  version: string;
}

export interface VisibilityEvent {
  minimized: boolean;
  visible: boolean;
  focused: boolean;
}

export type EventKind = "info" | "ok" | "warn" | "error";

export interface LogEvent {
  id: number;
  at: number;
  kind: EventKind;
  text: string;
}
