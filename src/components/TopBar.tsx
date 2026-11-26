import { AlertTriangle, Layers, RefreshCw, ShieldAlert, ShieldCheck } from "lucide-react";
import type { MutexStatus } from "../types";

interface TopBarProps {
  mutexStatus: MutexStatus | null;
  onRefreshMutex: () => void;
  onReacquireMutex: () => void;
  reacquiring: boolean;
}

export function TopBar({
  mutexStatus,
  onRefreshMutex,
  onReacquireMutex,
  reacquiring,
}: TopBarProps) {
  const isActive = mutexStatus?.active ?? false;
  const showStartupWarning = mutexStatus != null && !mutexStatus.active;

  return (
    <header className="shrink-0 border-b border-zinc-800/80 bg-zinc-950/90 backdrop-blur">
      <div className="flex items-center justify-between px-5 py-3">
        {/* Brand */}
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-emerald-500/15 ring-1 ring-emerald-500/30">
            <Layers className="h-5 w-5 text-emerald-400" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-base font-semibold tracking-tight text-zinc-100">RoMulti</h1>
              <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-zinc-400">
                v1.0
              </span>
            </div>
            <p className="text-xs text-zinc-500">Roblox Multi-Instance Launcher &amp; Account Manager</p>
          </div>
        </div>

        {/* Mutex Status Controls */}
        <div className="flex items-center gap-2.5">
          {mutexStatus ? (
            <div
              className={`flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-medium ${
                isActive
                  ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
                  : "border-red-500/30 bg-red-500/10 text-red-300"
              }`}
              title={`Event: ${mutexStatus.eventAcquired ? "OK" : "Failed"} | Mutex: ${mutexStatus.mutexAcquired ? "OK" : "Failed"} | Already existed: ${mutexStatus.alreadyExisted ? "Yes" : "No"}`}
            >
              {isActive ? (
                <>
                  <span className="relative flex h-2 w-2">
                    <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75" />
                    <span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-400" />
                  </span>
                  <ShieldCheck className="h-3.5 w-3.5" />
                  <span>Multi-Instance Mutex Active</span>
                </>
              ) : (
                <>
                  <span className="h-2 w-2 rounded-full bg-red-400" />
                  <ShieldAlert className="h-3.5 w-3.5" />
                  <span>
                    {mutexStatus.alreadyExisted
                      ? "Mutex Conflict Detected"
                      : "Multi-Instance Mutex Inactive"}
                  </span>
                </>
              )}
            </div>
          ) : (
            <div className="flex items-center gap-2 rounded-full border border-zinc-800 bg-zinc-900 px-3 py-1.5 text-xs text-zinc-400">
              Checking mutex status...
            </div>
          )}

          <button
            type="button"
            onClick={onRefreshMutex}
            className="flex h-8 w-8 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900 text-zinc-400 transition hover:border-zinc-700 hover:text-zinc-200"
            title="Refresh mutex & process status"
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      {/* Startup / Mutex Warning Banner */}
      {showStartupWarning && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-amber-500/20 bg-amber-500/10 px-5 py-2.5 text-xs text-amber-200">
          <div className="flex items-center gap-2.5">
            <AlertTriangle className="h-4 w-4 shrink-0 text-amber-400" />
            <span>
              {mutexStatus.robloxRunningAtStartup || mutexStatus.robloxRunningNow ? (
                <>
                  <strong>RobloxPlayerBeta.exe</strong> was detected running before RoMulti acquired the singleton lock. Close all running Roblox windows and click{" "}
                  <strong>Re-acquire Mutex</strong> so multiple instances can open reliably.
                </>
              ) : mutexStatus.error ? (
                <>Mutex initialization warning: {mutexStatus.error}</>
              ) : (
                <>
                  The Roblox singleton event/mutex already existed when RoMulti started. Close any other multi-instance tools or Roblox processes, then re-acquire.
                </>
              )}
            </span>
          </div>
          <button
            type="button"
            disabled={reacquiring}
            onClick={onReacquireMutex}
            className="shrink-0 rounded-md border border-amber-400/40 bg-amber-500/20 px-3 py-1 text-xs font-medium text-amber-100 transition hover:bg-amber-500/30 disabled:opacity-50"
          >
            {reacquiring ? "Resetting..." : "Close Roblox & Re-acquire"}
          </button>
        </div>
      )}
    </header>
  );
}
