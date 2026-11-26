import {
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Clock,
  Cpu,
  Gamepad2,
  Gauge,
  LayoutGrid,
  Link2,
  Loader2,
  Play,
  RotateCw,
  Square,
  Terminal,
  XCircle,
} from "lucide-react";
import { useState } from "react";
import type {
  AutoRejoinConfig,
  GameSearchResult,
  LaunchProgressEvent,
  LogEntry,
  PerformanceSettings,
} from "../types";

interface LaunchBarProps {
  selectedGame: GameSearchResult | null;
  hasPrivateServer: boolean;
  vipLinkValid: boolean;
  selectedCount: number;
  delaySeconds: number;
  isLaunching: boolean;
  currentProgress: LaunchProgressEvent | null;
  logs: LogEntry[];
  rejoinConfig: AutoRejoinConfig;
  performance: PerformanceSettings;
  applyingPerf: boolean;
  arranging: boolean;
  onDelayChange: (seconds: number) => void;
  onLaunch: () => void;
  onCancelLaunch: () => void;
  onClearLogs: () => void;
  onUpdateRejoinConfig: (next: AutoRejoinConfig) => void;
  onStopAutoRejoin: () => void;
  onArrangeWindows: () => void;
  onApplyPerformance: (next: PerformanceSettings) => void;
}

const FPS_OPTIONS: { label: string; value: string }[] = [
  { label: "Default", value: "default" },
  { label: "15 FPS (AFK)", value: "15" },
  { label: "30 FPS", value: "30" },
  { label: "60 FPS", value: "60" },
  { label: "240 FPS (Unlock)", value: "240" },
];

export function LaunchBar({
  selectedGame,
  hasPrivateServer,
  vipLinkValid,
  selectedCount,
  delaySeconds,
  isLaunching,
  currentProgress,
  logs,
  rejoinConfig,
  performance,
  applyingPerf,
  arranging,
  onDelayChange,
  onLaunch,
  onCancelLaunch,
  onClearLogs,
  onUpdateRejoinConfig,
  onStopAutoRejoin,
  onArrangeWindows,
  onApplyPerformance,
}: LaunchBarProps) {
  const [logExpanded, setLogExpanded] = useState(false);

  const canLaunch =
    selectedGame !== null && selectedCount > 0 && vipLinkValid && !isLaunching;
  const progressPercent =
    currentProgress && currentProgress.total > 0
      ? Math.round((currentProgress.index / currentProgress.total) * 100)
      : 0;

  const fpsSelectValue =
    performance.fpsCap === null ? "default" : String(performance.fpsCap);

  return (
    <footer className="shrink-0 border-t border-zinc-800/80 bg-zinc-950/95">
      {/* Live Launch Status / Expandable Log Drawer */}
      {(logs.length > 0 || isLaunching) && (
        <div className="border-b border-zinc-800/80 bg-zinc-900/60">
          <div className="flex items-center justify-between px-5 py-1.5 text-xs">
            <button
              type="button"
              onClick={() => setLogExpanded((v) => !v)}
              className="flex min-w-0 flex-1 items-center gap-2 text-left text-zinc-300 hover:text-white"
            >
              <Terminal className="h-3.5 w-3.5 shrink-0 text-zinc-500" />
              {isLaunching && currentProgress ? (
                <span className="flex min-w-0 items-center gap-2">
                  <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-emerald-400" />
                  <span className="truncate font-medium text-emerald-300">
                    {currentProgress.message}
                  </span>
                  <span className="shrink-0 text-zinc-500">({progressPercent}%)</span>
                </span>
              ) : (
                <span className="truncate text-zinc-400">
                  {logs[0]?.message ?? "Launch activity log"}
                </span>
              )}
              {logExpanded ? (
                <ChevronDown className="h-3.5 w-3.5 shrink-0 text-zinc-500" />
              ) : (
                <ChevronUp className="h-3.5 w-3.5 shrink-0 text-zinc-500" />
              )}
            </button>

            {logs.length > 0 && !isLaunching && (
              <button
                type="button"
                onClick={onClearLogs}
                className="ml-3 shrink-0 text-[11px] text-zinc-500 hover:text-zinc-300"
              >
                Clear
              </button>
            )}
          </div>

          {isLaunching && (
            <div className="h-0.5 w-full bg-zinc-800">
              <div
                className="h-full bg-emerald-500 transition-all duration-300"
                style={{ width: `${Math.max(8, progressPercent)}%` }}
              />
            </div>
          )}

          {logExpanded && logs.length > 0 && (
            <div className="max-h-36 overflow-y-auto border-t border-zinc-800/60 px-5 py-2 font-mono text-[11px]">
              {logs.map((entry) => (
                <div key={entry.id} className="flex items-start gap-2 py-0.5">
                  <span className="shrink-0 text-zinc-600">{entry.timestamp}</span>
                  {entry.status === "launched" && (
                    <CheckCircle2 className="mt-0.5 h-3 w-3 shrink-0 text-emerald-400" />
                  )}
                  {entry.status === "error" && (
                    <XCircle className="mt-0.5 h-3 w-3 shrink-0 text-red-400" />
                  )}
                  {entry.status === "launching" && (
                    <Loader2 className="mt-0.5 h-3 w-3 shrink-0 animate-spin text-sky-400" />
                  )}
                  {entry.status === "waiting" && (
                    <Clock className="mt-0.5 h-3 w-3 shrink-0 text-amber-400" />
                  )}
                  <span
                    className={
                      entry.status === "error"
                        ? "text-red-300"
                        : entry.status === "launched"
                          ? "text-emerald-300"
                          : entry.status === "waiting"
                            ? "text-amber-300"
                            : "text-zinc-300"
                    }
                  >
                    {entry.message}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Session Settings Bar: Auto Rejoin | Window Grid | FPS Cap & Low CPU */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-zinc-800/70 bg-zinc-900/40 px-5 py-2 text-xs">
        {/* Auto Rejoin Group */}
        <div className="flex flex-wrap items-center gap-2.5">
          <label className="flex cursor-pointer items-center gap-2 select-none">
            <button
              type="button"
              role="switch"
              aria-checked={rejoinConfig.enabled}
              onClick={() =>
                onUpdateRejoinConfig({
                  ...rejoinConfig,
                  enabled: !rejoinConfig.enabled,
                })
              }
              className={`relative inline-flex h-4.5 w-8 items-center rounded-full transition ${
                rejoinConfig.enabled ? "bg-emerald-500" : "bg-zinc-700"
              }`}
            >
              <span
                className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white transition ${
                  rejoinConfig.enabled ? "translate-x-4" : "translate-x-0.5"
                }`}
              />
            </button>
            <RotateCw
              className={`h-3.5 w-3.5 ${
                rejoinConfig.enabled ? "text-emerald-400" : "text-zinc-500"
              }`}
            />
            <span className="font-medium text-zinc-200">Auto Rejoin</span>
          </label>

          <div
            className="flex items-center gap-1 rounded-md border border-zinc-800 bg-zinc-950/80 px-2 py-0.5"
            title="Rejoins each account every N minutes (prevents 20-min AFK kick) and recovers crashes/disconnects automatically"
          >
            <span className="text-[11px] text-zinc-500">Every</span>
            <input
              type="number"
              min={1}
              max={240}
              value={rejoinConfig.intervalMinutes}
              onChange={(e) => {
                const parsed = Number(e.target.value);
                if (!Number.isNaN(parsed)) {
                  onUpdateRejoinConfig({
                    ...rejoinConfig,
                    intervalMinutes: Math.max(1, Math.min(240, parsed)),
                  });
                }
              }}
              className="w-9 bg-transparent text-center font-mono text-xs text-zinc-100 outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
            />
            <div className="flex flex-col leading-none">
              <button
                type="button"
                onClick={() =>
                  onUpdateRejoinConfig({
                    ...rejoinConfig,
                    intervalMinutes: Math.min(240, rejoinConfig.intervalMinutes + 1),
                  })
                }
                className="flex h-2.5 w-3.5 items-center justify-center rounded-t text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-200"
                aria-label="Increase minutes"
              >
                <ChevronUp className="h-2.5 w-2.5" />
              </button>
              <button
                type="button"
                onClick={() =>
                  onUpdateRejoinConfig({
                    ...rejoinConfig,
                    intervalMinutes: Math.max(1, rejoinConfig.intervalMinutes - 1),
                  })
                }
                className="flex h-2.5 w-3.5 items-center justify-center rounded-b text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-200"
                aria-label="Decrease minutes"
              >
                <ChevronDown className="h-2.5 w-2.5" />
              </button>
            </div>
            <span className="text-[11px] text-zinc-500">min</span>
          </div>

          {rejoinConfig.enabled && (
            <button
              type="button"
              onClick={onStopAutoRejoin}
              className="inline-flex items-center gap-1 rounded-md border border-red-500/35 bg-red-500/15 px-2 py-0.5 text-[11px] font-medium text-red-200 transition hover:bg-red-500/25"
            >
              <Square className="h-2.5 w-2.5 fill-current" />
              Stop Auto Rejoin
            </button>
          )}
        </div>

        {/* Window Grid Arrangement Group */}
        <div className="flex items-center gap-2.5">
          <button
            type="button"
            disabled={arranging}
            onClick={onArrangeWindows}
            className="inline-flex items-center gap-1.5 rounded-lg border border-zinc-700/90 bg-zinc-800/80 px-2.5 py-1 text-xs font-medium text-zinc-200 transition hover:border-zinc-600 hover:bg-zinc-700 disabled:opacity-50"
            title="Arrange all running Roblox windows evenly into a grid"
          >
            {arranging ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin text-emerald-400" />
            ) : (
              <LayoutGrid className="h-3.5 w-3.5 text-emerald-400" />
            )}
            <span>Arrange Windows</span>
          </button>

          <label className="flex cursor-pointer items-center gap-1.5 text-zinc-400 select-none hover:text-zinc-200">
            <input
              type="checkbox"
              checked={rejoinConfig.autoArrange}
              onChange={(e) =>
                onUpdateRejoinConfig({
                  ...rejoinConfig,
                  autoArrange: e.target.checked,
                })
              }
              className="h-3.5 w-3.5 cursor-pointer rounded border-zinc-700 bg-zinc-900 accent-emerald-500"
            />
            <span>Auto-arrange</span>
          </label>
        </div>

        {/* FPS Cap & Low CPU Mode Group */}
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            <Gauge className="h-3.5 w-3.5 text-zinc-400" />
            <span className="text-zinc-400">FPS:</span>
            <select
              value={fpsSelectValue}
              disabled={applyingPerf}
              onChange={(e) => {
                const raw = e.target.value;
                onApplyPerformance({
                  ...performance,
                  fpsCap: raw === "default" ? null : Number(raw),
                });
              }}
              className="rounded-md border border-zinc-700/90 bg-zinc-900 px-2 py-1 text-xs text-zinc-100 outline-none focus:border-emerald-500/60 disabled:opacity-50"
            >
              {FPS_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>

          <label
            className="flex cursor-pointer items-center gap-2 select-none"
            title="Sets BELOW_NORMAL process priority on Roblox instances and writes lightweight FFlags to ClientAppSettings.json"
          >
            <button
              type="button"
              role="switch"
              disabled={applyingPerf}
              aria-checked={performance.lowCpuMode}
              onClick={() =>
                onApplyPerformance({
                  ...performance,
                  lowCpuMode: !performance.lowCpuMode,
                })
              }
              className={`relative inline-flex h-4.5 w-8 items-center rounded-full transition disabled:opacity-50 ${
                performance.lowCpuMode ? "bg-amber-500" : "bg-zinc-700"
              }`}
            >
              <span
                className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white transition ${
                  performance.lowCpuMode ? "translate-x-4" : "translate-x-0.5"
                }`}
              />
            </button>
            <Cpu
              className={`h-3.5 w-3.5 ${
                performance.lowCpuMode ? "text-amber-400" : "text-zinc-500"
              }`}
            />
            <span
              className={`font-medium ${
                performance.lowCpuMode ? "text-amber-200" : "text-zinc-300"
              }`}
            >
              Low CPU Mode
            </span>
          </label>
        </div>
      </div>

      {/* Main Action Controls */}
      <div className="flex flex-wrap items-center justify-between gap-4 px-5 py-3">
        {/* Selected Game Preview */}
        <div className="flex min-w-0 flex-1 items-center gap-3">
          {selectedGame ? (
            <>
              <div className="h-11 w-11 shrink-0 overflow-hidden rounded-lg bg-zinc-800 ring-1 ring-zinc-700">
                {selectedGame.iconUrl ? (
                  <img
                    src={selectedGame.iconUrl}
                    alt={selectedGame.name}
                    className="h-full w-full object-cover"
                  />
                ) : (
                  <div className="flex h-full w-full items-center justify-center text-zinc-500">
                    <Gamepad2 className="h-5 w-5" />
                  </div>
                )}
              </div>
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <p className="truncate text-sm font-semibold text-zinc-100">
                    {selectedGame.name}
                  </p>
                  {hasPrivateServer && vipLinkValid && (
                    <span className="inline-flex shrink-0 items-center gap-1 rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-medium text-emerald-300">
                      <Link2 className="h-2.5 w-2.5" />
                      VIP Server
                    </span>
                  )}
                </div>
                <p className="truncate text-xs text-zinc-500">
                  PlaceId: <span className="font-mono text-zinc-400">{selectedGame.placeId}</span>{" "}
                  • {selectedGame.playing.toLocaleString()} playing
                </p>
              </div>
            </>
          ) : (
            <div className="flex items-center gap-3 text-zinc-500">
              <div className="flex h-11 w-11 items-center justify-center rounded-lg border border-dashed border-zinc-800 bg-zinc-900/50">
                <Gamepad2 className="h-5 w-5" />
              </div>
              <div>
                <p className="text-sm font-medium text-zinc-400">No game selected</p>
                <p className="text-xs text-zinc-600">Pick a game on the right to enable launch</p>
              </div>
            </div>
          )}
        </div>

        {/* Delay Slider + Launch Button */}
        <div className="flex shrink-0 items-center gap-4">
          {/* Delay control */}
          <div className="flex items-center gap-3 rounded-xl border border-zinc-800/80 bg-zinc-900/60 px-3.5 py-2">
            <Clock className="h-4 w-4 text-zinc-400" />
            <div className="flex flex-col">
              <div className="flex items-center justify-between gap-4 text-[11px]">
                <span className="text-zinc-400">Launch Delay</span>
                <span className="font-mono font-semibold text-emerald-400">{delaySeconds}s</span>
              </div>
              <input
                type="range"
                min={2}
                max={20}
                step={1}
                value={delaySeconds}
                disabled={isLaunching}
                onChange={(e) => onDelayChange(Number(e.target.value))}
                className="mt-1 h-1.5 w-28 cursor-pointer accent-emerald-500 disabled:opacity-50"
              />
            </div>
          </div>

          {/* Join / Stop Button */}
          {isLaunching ? (
            <button
              type="button"
              onClick={onCancelLaunch}
              className="flex min-w-[180px] items-center justify-center gap-2 rounded-xl bg-red-600 px-5 py-3 text-sm font-semibold text-white shadow-lg shadow-red-950/40 transition hover:bg-red-500 active:scale-[0.98]"
            >
              <Square className="h-4 w-4 fill-current" />
              <span>Stop Launching</span>
            </button>
          ) : (
            <button
              type="button"
              onClick={onLaunch}
              disabled={!canLaunch}
              className="flex min-w-[180px] items-center justify-center gap-2 rounded-xl bg-emerald-600 px-5 py-3 text-sm font-semibold text-white shadow-lg shadow-emerald-950/40 transition hover:bg-emerald-500 active:scale-[0.98] disabled:cursor-not-allowed disabled:bg-zinc-800 disabled:text-zinc-500 disabled:shadow-none"
            >
              <Play className="h-4 w-4 fill-current" />
              <span>
                Join ({selectedCount} {selectedCount === 1 ? "account" : "accounts"})
              </span>
            </button>
          )}
        </div>
      </div>
    </footer>
  );
}
