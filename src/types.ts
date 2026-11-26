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
  tags: string[];
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

export interface PublicServerInfo {
  id: string;
  playing: number;
  maxPlayers: number;
  ping: number | null;
  fps: number | null;
}

export interface PublicServersPage {
  servers: PublicServerInfo[];
  nextPageCursor: string | null;
}

export type MuteMode = "off" | "mute_all" | "mute_except_first";

export interface FavoriteGame {
  placeId: number;
  universeId: number;
  name: string;
  iconUrl: string | null;
  privateServerLink: string | null;
}

export interface LaunchProfile {
  id: string;
  name: string;
  userIds: number[];
  placeId: number;
  universeId?: number | null;
  gameName: string;
  gameIconUrl: string | null;
  privateServerLink: string | null;
  jobId: string | null;
  joinSameServer: boolean;
  delaySeconds: number;
  fpsCap: number | null;
  lowCpuMode: boolean;
  autoRejoinEnabled: boolean;
  autoRejoinIntervalMinutes: number;
  autoArrange: boolean;
  monitorId: string | null;
  muteMode: MuteMode;
}

export interface AppSettings {
  minimizeToTray: boolean;
  discordWebhookUrl: string;
  notifyExpiredCookie: boolean;
  notifyRejoinFailed: boolean;
  notifyEveryRejoin: boolean;
  defaultMonitorId: string | null;
  muteMode: MuteMode;
}

export interface AppConfig {
  favorites: FavoriteGame[];
  profiles: LaunchProfile[];
  appSettings: AppSettings;
}

export interface MonitorInfo {
  id: string;
  name: string;
  isPrimary: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AudioApplyResult {
  mode: MuteMode;
  mutedCount: number;
  unmutedCount: number;
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
