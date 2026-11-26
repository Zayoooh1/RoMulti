import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AccountsPanel, type AccountVerifyState } from "./components/AccountsPanel";
import {
  GameSearchPanel,
  validatePrivateServerLink,
} from "./components/GameSearchPanel";
import { LaunchBar } from "./components/LaunchBar";
import { TopBar } from "./components/TopBar";
import type {
  ActiveSessionInfo,
  ApplyPerformanceResult,
  ArrangeWindowsResult,
  AuthErrorPayload,
  AuthStatusPayload,
  AutoRejoinConfig,
  GameSearchResult,
  LaunchCompleteEvent,
  LaunchProgressEvent,
  LogEntry,
  MutexStatus,
  PerformanceSettings,
  PublicAccount,
  RejoinTriggeredEvent,
} from "./types";

function nowTime(): string {
  return new Date().toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function App() {
  // Mutex status
  const [mutexStatus, setMutexStatus] = useState<MutexStatus | null>(null);
  const [reacquiring, setReacquiring] = useState(false);

  // Accounts & active sessions state
  const [accounts, setAccounts] = useState<PublicAccount[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [verifyStates, setVerifyStates] = useState<Record<number, AccountVerifyState>>({});
  const [sessions, setSessions] = useState<Record<number, ActiveSessionInfo>>({});
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [authMessage, setAuthMessage] = useState<string | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);

  // Game search & Private Server state
  const [query, setQuery] = useState("");
  const [jobId, setJobId] = useState("");
  const [privateServerLink, setPrivateServerLink] = useState("");
  const [results, setResults] = useState<GameSearchResult[]>([]);
  const [selectedGame, setSelectedGame] = useState<GameSearchResult | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);

  // Launch, Auto Rejoin, Grid & Performance state
  const [delaySeconds, setDelaySeconds] = useState(6);
  const [isLaunching, setIsLaunching] = useState(false);
  const [currentProgress, setCurrentProgress] = useState<LaunchProgressEvent | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [rejoinConfig, setRejoinConfig] = useState<AutoRejoinConfig>({
    enabled: false,
    intervalMinutes: 15,
    autoArrange: false,
  });
  const [performance, setPerformance] = useState<PerformanceSettings>({
    fpsCap: null,
    lowCpuMode: false,
  });
  const [applyingPerf, setApplyingPerf] = useState(false);
  const [arranging, setArranging] = useState(false);

  const searchRequestId = useRef(0);

  const vipValidation = useMemo(
    () => validatePrivateServerLink(privateServerLink),
    [privateServerLink],
  );

  const appendLog = useCallback(
    (status: LogEntry["status"], message: string, username?: string) => {
      setLogs((prev) => [
        {
          id: `${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
          timestamp: nowTime(),
          status,
          username,
          message,
        },
        ...prev.slice(0, 99),
      ]);
    },
    [],
  );

  const loadMutexStatus = useCallback(async () => {
    try {
      const status = await invoke<MutexStatus>("get_mutex_status");
      setMutexStatus(status);
      if (status.autoClosedCount > 0 && status.active) {
        appendLog(
          "info",
          `Automatically closed ${status.autoClosedCount} existing Roblox process(es) and acquired multi-instance mutex.`,
        );
      }
    } catch (err) {
      appendLog("error", `Failed to read mutex status: ${String(err)}`);
    }
  }, [appendLog]);

  const loadAccounts = useCallback(async () => {
    try {
      const list = await invoke<PublicAccount[]>("get_accounts");
      setAccounts(list);
      setSelectedIds((prev) => {
        const validIds = new Set(list.map((a) => a.userId));
        const next = new Set<number>();
        prev.forEach((id) => {
          if (validIds.has(id)) next.add(id);
        });
        return next;
      });
    } catch (err) {
      setAuthError(`Failed to load saved accounts: ${String(err)}`);
    }
  }, []);

  const loadInitialSessionState = useCallback(async () => {
    try {
      const [activeList, cfg, perf] = await Promise.all([
        invoke<ActiveSessionInfo[]>("get_active_sessions"),
        invoke<AutoRejoinConfig>("get_auto_rejoin_config"),
        invoke<PerformanceSettings>("get_performance_settings"),
      ]);
      const map: Record<number, ActiveSessionInfo> = {};
      for (const s of activeList) {
        map[s.userId] = s;
      }
      setSessions(map);
      setRejoinConfig(cfg);
      setPerformance(perf);
    } catch {
      // Non-fatal during initial bootstrap
    }
  }, []);

  // Initial load + Tauri event subscriptions
  useEffect(() => {
    loadMutexStatus();
    loadAccounts();
    loadInitialSessionState();

    const unlistenPromises = [
      listen<AuthStatusPayload>("auth-status", (event) => {
        setIsAuthenticating(true);
        setAuthMessage(event.payload.message);
      }),
      listen<PublicAccount>("account-added", (event) => {
        const added = event.payload;
        setIsAuthenticating(false);
        setAuthMessage(null);
        setAuthError(null);
        setAccounts((prev) => {
          const exists = prev.some((a) => a.userId === added.userId);
          return exists
            ? prev.map((a) => (a.userId === added.userId ? added : a))
            : [...prev, added];
        });
        setSelectedIds((prev) => {
          const next = new Set(prev);
          next.add(added.userId);
          return next;
        });
        setVerifyStates((prev) => ({
          ...prev,
          [added.userId]: { status: "valid" },
        }));
        appendLog("launched", `Account @${added.username} added and encrypted.`);
      }),
      listen("auth-cancelled", () => {
        setIsAuthenticating(false);
        setAuthMessage(null);
      }),
      listen<AuthErrorPayload>("auth-error", (event) => {
        setIsAuthenticating(false);
        setAuthMessage(null);
        setAuthError(event.payload.error);
      }),
      listen<LaunchProgressEvent>("launch-progress", (event) => {
        const payload = event.payload;
        setCurrentProgress(payload);
        appendLog(payload.status, payload.message, payload.username);
        if (payload.status === "error" && payload.message.toLowerCase().includes("expired")) {
          setVerifyStates((prev) => ({
            ...prev,
            [payload.userId]: { status: "invalid", error: payload.message },
          }));
        }
      }),
      listen<LaunchCompleteEvent>("launch-complete", (event) => {
        const { launched, failed, cancelled } = event.payload;
        setIsLaunching(false);
        setCurrentProgress(null);
        const summary = cancelled
          ? `Launch cancelled — ${launched} launched, ${failed} failed.`
          : `Launch finished — ${launched} launched${failed > 0 ? `, ${failed} failed` : ""}.`;
        appendLog(failed > 0 ? "error" : "launched", summary);
        loadMutexStatus();
      }),
      listen<ActiveSessionInfo>("session-updated", (event) => {
        const session = event.payload;
        setSessions((prev) => ({ ...prev, [session.userId]: session }));
        if (session.status === "error" && session.consecutiveFailures >= 3) {
          appendLog("error", `[${session.username}] ${session.lastMessage}`, session.username);
        }
      }),
      listen<RejoinTriggeredEvent>("rejoin-triggered", (event) => {
        const { username, reason, attempt } = event.payload;
        appendLog(
          "waiting",
          `Auto Rejoin triggered for @${username} (attempt ${attempt}/3): ${reason}`,
          username,
        );
      }),
      listen<AutoRejoinConfig>("rejoin-status", (event) => {
        setRejoinConfig(event.payload);
      }),
      listen<MutexStatus>("mutex-status", (event) => {
        setMutexStatus(event.payload);
      }),
    ];

    return () => {
      unlistenPromises.forEach((p) => {
        p.then((unlisten) => unlisten()).catch(() => {});
      });
    };
  }, [appendLog, loadAccounts, loadInitialSessionState, loadMutexStatus]);

  // Game search handler
  const executeSearch = useCallback(async (searchText: string) => {
    const trimmed = searchText.trim();
    const reqId = ++searchRequestId.current;

    if (!trimmed) {
      setResults([]);
      setSearchError(null);
      setHasSearched(false);
      setSearching(false);
      return;
    }

    setSearching(true);
    setSearchError(null);
    setHasSearched(true);

    try {
      const games = await invoke<GameSearchResult[]>("search_games", { query: trimmed });
      if (reqId !== searchRequestId.current) return;
      setResults(games);
      if (games.length === 1 && /^\d+$/.test(trimmed)) {
        setSelectedGame(games[0]);
      }
    } catch (err) {
      if (reqId !== searchRequestId.current) return;
      setResults([]);
      setSearchError(String(err));
    } finally {
      if (reqId === searchRequestId.current) {
        setSearching(false);
      }
    }
  }, []);

  // Debounced auto-search when typing
  useEffect(() => {
    const trimmed = query.trim();
    if (!trimmed) {
      setResults([]);
      setSearchError(null);
      setHasSearched(false);
      return;
    }
    const timer = window.setTimeout(() => {
      executeSearch(trimmed);
    }, 450);

    return () => window.clearTimeout(timer);
  }, [query, executeSearch]);

  // Actions
  const handleReacquireMutex = async () => {
    setReacquiring(true);
    try {
      const status = await invoke<MutexStatus>("reinitialize_mutex");
      setMutexStatus(status);
      appendLog(
        status.active ? "launched" : "error",
        status.active
          ? "Re-acquired Roblox singleton event & mutex cleanly."
          : "Mutex re-acquisition detected an existing lock or Roblox process.",
      );
    } catch (err) {
      appendLog("error", `Mutex re-acquisition error: ${String(err)}`);
    } finally {
      setReacquiring(false);
    }
  };

  const handleAddAccount = async () => {
    setAuthError(null);
    setIsAuthenticating(true);
    setAuthMessage("Opening Roblox sign-in window...");
    try {
      await invoke("open_auth_window");
    } catch (err) {
      setIsAuthenticating(false);
      setAuthMessage(null);
      setAuthError(String(err));
    }
  };

  const handleRemoveAccount = async (userId: number) => {
    try {
      await invoke("remove_account", { userId });
      setAccounts((prev) => prev.filter((a) => a.userId !== userId));
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.delete(userId);
        return next;
      });
      setSessions((prev) => {
        const next = { ...prev };
        delete next[userId];
        return next;
      });
    } catch (err) {
      setAuthError(`Failed to remove account: ${String(err)}`);
    }
  };

  const handleVerifyAccount = async (userId: number) => {
    setVerifyStates((prev) => ({ ...prev, [userId]: { status: "checking" } }));
    try {
      const updated = await invoke<PublicAccount>("verify_account", { userId });
      setAccounts((prev) => prev.map((a) => (a.userId === updated.userId ? updated : a)));
      setVerifyStates((prev) => ({ ...prev, [userId]: { status: "valid" } }));
    } catch (err) {
      setVerifyStates((prev) => ({
        ...prev,
        [userId]: { status: "invalid", error: String(err) },
      }));
    }
  };

  const handleToggleSelect = (userId: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(userId)) {
        next.delete(userId);
      } else {
        next.add(userId);
      }
      return next;
    });
  };

  const handleSelectAll = () => {
    setSelectedIds(new Set(accounts.map((a) => a.userId)));
  };

  const handleSelectNone = () => {
    setSelectedIds(new Set());
  };

  const handleLaunch = async () => {
    if (!selectedGame || selectedIds.size === 0 || !vipValidation.valid) return;

    const orderedUserIds = accounts
      .filter((a) => selectedIds.has(a.userId))
      .map((a) => a.userId);

    const vipTrimmed = privateServerLink.trim() || null;

    setIsLaunching(true);
    appendLog(
      "info",
      `Starting launch of ${orderedUserIds.length} account(s) into "${selectedGame.name}"${vipTrimmed ? " (Private Server)" : ""} with ${delaySeconds}s delay...`,
    );

    try {
      await invoke("launch_accounts", {
        userIds: orderedUserIds,
        placeId: selectedGame.placeId,
        jobId: vipTrimmed ? null : jobId.trim() || null,
        privateServerLink: vipTrimmed,
        delaySeconds,
        autoArrange: rejoinConfig.autoArrange,
      });
    } catch (err) {
      setIsLaunching(false);
      appendLog("error", `Failed to start launch: ${String(err)}`);
    }
  };

  const handleCancelLaunch = async () => {
    try {
      await invoke("cancel_launch");
      appendLog("waiting", "Cancelling remaining launches...");
    } catch (err) {
      appendLog("error", `Failed to cancel launch: ${String(err)}`);
    }
  };

  const handleUpdateRejoinConfig = async (next: AutoRejoinConfig) => {
    setRejoinConfig(next);
    try {
      const applied = await invoke<AutoRejoinConfig>("set_auto_rejoin_config", {
        config: next,
      });
      setRejoinConfig(applied);
      if (applied.enabled !== rejoinConfig.enabled) {
        appendLog(
          "info",
          applied.enabled
            ? `Auto Rejoin enabled (every ${applied.intervalMinutes} min + instant crash/kick recovery).`
            : "Auto Rejoin disabled.",
        );
      }
    } catch (err) {
      appendLog("error", `Failed to update Auto Rejoin config: ${String(err)}`);
    }
  };

  const handleStopAutoRejoin = async () => {
    try {
      const applied = await invoke<AutoRejoinConfig>("stop_auto_rejoin");
      setRejoinConfig(applied);
      appendLog("waiting", "Auto Rejoin stopped cleanly.");
    } catch (err) {
      appendLog("error", `Failed to stop Auto Rejoin: ${String(err)}`);
    }
  };

  const handleArrangeWindows = async () => {
    setArranging(true);
    try {
      const res = await invoke<ArrangeWindowsResult>("arrange_windows");
      if (res.arrangedCount === 0) {
        appendLog("info", "No visible Roblox windows found to arrange.");
      } else {
        appendLog(
          "launched",
          `Arranged ${res.arrangedCount} Roblox window(s) into a ${res.cols}×${res.rows} grid.`,
        );
      }
    } catch (err) {
      appendLog("error", `Failed to arrange windows: ${String(err)}`);
    } finally {
      setArranging(false);
    }
  };

  const handleApplyPerformance = async (next: PerformanceSettings) => {
    setPerformance(next);
    setApplyingPerf(true);
    try {
      const res = await invoke<ApplyPerformanceResult>("apply_performance_settings", {
        settings: next,
      });
      setPerformance(res.settings);
      if (res.warning) {
        appendLog("waiting", res.warning);
      } else {
        const fpsLabel =
          res.settings.fpsCap !== null
            ? `${res.settings.fpsCap} FPS`
            : res.settings.lowCpuMode
              ? "15 FPS (Low CPU)"
              : "Default FPS";
        appendLog(
          "launched",
          `Performance applied: ${fpsLabel}, Low CPU Mode ${res.settings.lowCpuMode ? "ON" : "OFF"} (${res.updatedProcesses} running process(es) updated).`,
        );
      }
    } catch (err) {
      appendLog("error", `Failed to apply performance settings: ${String(err)}`);
    } finally {
      setApplyingPerf(false);
    }
  };

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-zinc-950 text-zinc-100">
      <TopBar
        mutexStatus={mutexStatus}
        onRefreshMutex={loadMutexStatus}
        onReacquireMutex={handleReacquireMutex}
        reacquiring={reacquiring}
      />

      <main className="flex min-h-0 flex-1 overflow-hidden">
        <AccountsPanel
          accounts={accounts}
          selectedIds={selectedIds}
          verifyStates={verifyStates}
          sessions={sessions}
          isAuthenticating={isAuthenticating}
          authMessage={authMessage}
          authError={authError}
          disabled={isLaunching}
          onAddAccount={handleAddAccount}
          onToggleSelect={handleToggleSelect}
          onSelectAll={handleSelectAll}
          onSelectNone={handleSelectNone}
          onRemoveAccount={handleRemoveAccount}
          onVerifyAccount={handleVerifyAccount}
          onDismissAuthError={() => setAuthError(null)}
        />

        <GameSearchPanel
          query={query}
          jobId={jobId}
          privateServerLink={privateServerLink}
          vipValidation={vipValidation}
          results={results}
          selectedGame={selectedGame}
          searching={searching}
          searchError={searchError}
          hasSearched={hasSearched}
          onQueryChange={setQuery}
          onJobIdChange={setJobId}
          onPrivateServerLinkChange={setPrivateServerLink}
          onSearchSubmit={() => executeSearch(query)}
          onSelectGame={setSelectedGame}
        />
      </main>

      <LaunchBar
        selectedGame={selectedGame}
        hasPrivateServer={privateServerLink.trim().length > 0}
        vipLinkValid={vipValidation.valid}
        selectedCount={selectedIds.size}
        delaySeconds={delaySeconds}
        isLaunching={isLaunching}
        currentProgress={currentProgress}
        logs={logs}
        rejoinConfig={rejoinConfig}
        performance={performance}
        applyingPerf={applyingPerf}
        arranging={arranging}
        onDelayChange={setDelaySeconds}
        onLaunch={handleLaunch}
        onCancelLaunch={handleCancelLaunch}
        onClearLogs={() => setLogs([])}
        onUpdateRejoinConfig={handleUpdateRejoinConfig}
        onStopAutoRejoin={handleStopAutoRejoin}
        onArrangeWindows={handleArrangeWindows}
        onApplyPerformance={handleApplyPerformance}
      />
    </div>
  );
}

export default App;
