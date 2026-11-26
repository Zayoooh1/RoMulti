export interface MutexStatus {
  active: boolean;
  eventAcquired: boolean;
  mutexAcquired: boolean;
  alreadyExisted: boolean;
  robloxRunningAtStartup: boolean;
  robloxRunningNow: boolean;
  autoClosedCount: number;
  error: string | null;
}

export interface PublicAccount {
  userId: number;
  username: string;
  displayName: string;
  avatarUrl: string;
  addedAt: number;
}

export interface GameSearchResult {
  universeId: number;
  placeId: number;
  name: string;
  creatorName: string;
  playing: number;
  visits: number;
  maxPlayers: number;
  iconUrl: string | null;
}

export interface AuthStatusPayload {
  message: string;
}

export interface AuthErrorPayload {
  error: string;
}

export interface LaunchProgressEvent {
  index: number;
  total: number;
  userId: number;
  username: string;
  status: "launching" | "launched" | "waiting" | "error";
  message: string;
}

export interface LaunchCompleteEvent {
  launched: number;
  failed: number;
  cancelled: boolean;
}

export type SessionStatus = "launching" | "in_game" | "rejoining" | "error" | "stopped";

export interface ActiveSessionInfo {
  userId: number;
  username: string;
  placeId: number;
  jobId: string | null;
  privateServerLink: string | null;
  pid: number | null;
  status: SessionStatus;
  consecutiveFailures: number;
  lastMessage: string;
}

export interface AutoRejoinConfig {
  enabled: boolean;
  intervalMinutes: number;
  autoArrange: boolean;
}

export interface RejoinTriggeredEvent {
  userId: number;
  username: string;
  reason: string;
  attempt: number;
}

export interface ArrangeWindowsResult {
  arrangedCount: number;
  cols: number;
  rows: number;
}

export interface PerformanceSettings {
  fpsCap: number | null;
  lowCpuMode: boolean;
}

export interface ApplyPerformanceResult {
  settings: PerformanceSettings;
  clientSettingsPath: string | null;
  updatedProcesses: number;
  warning: string | null;
}

export interface LogEntry {
  id: string;
  timestamp: string;
  status: "launching" | "launched" | "waiting" | "error" | "info";
  username?: string;
  message: string;
}
