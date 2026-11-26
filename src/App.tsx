import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AccountsPanel, type AccountVerifyState } from "./components/AccountsPanel";
import {
  GameSearchPanel,
  validatePrivateServerLink,
} from "./components/GameSearchPanel";
import { LaunchBar } from "./components/LaunchBar";
import { ProfilesBar } from "./components/ProfilesBar";
import { SettingsModal } from "./components/SettingsModal";
import { TopBar } from "./components/TopBar";
import type {
  ActiveSessionInfo,
  AppConfig,
  ApplyPerformanceResult,
  AppSettings,
  ArrangeWindowsResult,
  AudioApplyResult,
  AuthErrorPayload,
  AuthStatusPayload,
  AutoRejoinConfig,
  FavoriteGame,
  GameSearchResult,
  LaunchCompleteEvent,
  LaunchProfile,
  LaunchProgressEvent,
  LogEntry,
  MonitorInfo,
  MuteMode,
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

const DEFAULT_APP_SETTINGS: AppSettings = {
  minimizeToTray: false,
  discordWebhookUrl: "",
  notifyExpiredCookie: true,
  notifyRejoinFailed: true,
  notifyEveryRejoin: false,
  defaultMonitorId: null,
  muteMode: "off",
};

export function App() {
  // Mutex status
  const [mutexStatus, setMutexStatus] = useState<MutexStatus | null>(null);
  const [reacquiring, setReacquiring] = useState(false);

  // Accounts & active sessions state
  const [accounts, setAccounts] = useState<PublicAccount[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [verifyStates, setVerifyStates] = useState<Record<number, AccountVerifyState>>({});
  const [sessions, setSessions] = useState<Record<number, ActiveSessionInfo>>({});
  const [mutedAccounts, setMutedAccounts] = useState<Record<number, boolean>>({});
  const [busySessions, setBusySessions] = useState<Record<number, string | undefined>>({});
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [authMessage, setAuthMessage] = useState<string | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);

  // Config: favorites, profiles, app settings
  const [appConfig, setAppConfig] = useState<AppConfig>({
    favorites: [],
    profiles: [],
    appSettings: DEFAULT_APP_SETTINGS,
  });
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Game search, Private Server & Join Same Server state
  const [query, setQuery] = useState("");
  const [jobId, setJobId] = useState("");
  const [privateServerLink, setPrivateServerLink] = useState("");
  const [joinSameServer, setJoinSameServer] = useState(false);
  const [results, setResults] = useState<GameSearchResult[]>([]);
  const [selectedGame, setSelectedGame] = useState<GameSearchResult | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);

  // Launch, Auto Rejoin, Grid, Monitors, Audio & Performance state
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
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [selectedMonitorId, setSelectedMonitorId] = useState<string | null>(null);
  const [muteMode, setMuteMode] = useState<MuteMode>("off");
  const [applyingPerf, setApplyingPerf] = useState(false);
  const [arranging, setArranging] = useState(false);
  const [applyingMute, setApplyingMute] = useState(false);

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
      const [activeList, cfg, perf, configData, monitorList] = await Promise.all([
        invoke<ActiveSessionInfo[]>("get_active_sessions"),
        invoke<AutoRejoinConfig>("get_auto_rejoin_config"),
        invoke<PerformanceSettings>("get_performance_settings"),
        invoke<AppConfig>("get_app_config"),
        invoke<MonitorInfo[]>("list_monitors"),
      ]);
      const map: Record<number, ActiveSessionInfo> = {};
      for (const s of activeList) {
        map[s.userId] = s;
      }
      setSessions(map);
      setRejoinConfig(cfg);
      setPerformance(perf);
      setAppConfig(configData);
      setMonitors(monitorList);
      setMuteMode(configData.appSettings.muteMode ?? "off");

      const savedMon = configData.appSettings.defaultMonitorId;
      if (savedMon && monitorList.some((m) => m.id === savedMon)) {
        setSelectedMonitorId(savedMon);
      } else {
        const primary = monitorList.find((m) => m.isPrimary) ?? monitorList[0];
        setSelectedMonitorId(primary?.id ?? null);
      }
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
      listen<MuteMode>("mute-mode-changed", (event) => {
        setMuteMode(event.payload);
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

  const handleUpdateTags = async (userId: number, tags: string[]) => {
    try {
      const updated = await invoke<PublicAccount>("update_account_tags", { userId, tags });
      setAccounts((prev) => prev.map((a) => (a.userId === updated.userId ? updated : a)));
    } catch (err) {
      appendLog("error", `Failed to update tags: ${String(err)}`);
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

  const handleSelectByTag = (tag: string) => {
    const matching = accounts
      .filter((a) => (a.tags ?? []).some((t) => t.toLowerCase() === tag.toLowerCase()))
      .map((a) => a.userId);
    setSelectedIds(new Set(matching));
  };

  // Per-instance session controls
  const handleFocusAccount = async (userId: number) => {
    setBusySessions((prev) => ({ ...prev, [userId]: "focus" }));
    try {
      await invoke("focus_account_window", { userId });
    } catch (err) {
      appendLog("error", `Focus failed: ${String(err)}`);
    } finally {
      setBusySessions((prev) => ({ ...prev, [userId]: undefined }));
    }
  };

  const handleKillAccount = async (userId: number) => {
    setBusySessions((prev) => ({ ...prev, [userId]: "kill" }));
    try {
      await invoke("kill_account_session", { userId });
      appendLog("waiting", `Stopped instance for user #${userId}.`);
    } catch (err) {
      appendLog("error", `Kill failed: ${String(err)}`);
    } finally {
      setBusySessions((prev) => ({ ...prev, [userId]: undefined }));
    }
  };

  const handleRejoinAccount = async (userId: number) => {
    setBusySessions((prev) => ({ ...prev, [userId]: "rejoin" }));
    try {
      const updated = await invoke<ActiveSessionInfo>("rejoin_single_account", { userId });
      appendLog("launched", updated.lastMessage, updated.username);
    } catch (err) {
      appendLog("error", `Rejoin failed: ${String(err)}`);
    } finally {
      setBusySessions((prev) => ({ ...prev, [userId]: undefined }));
    }
  };

  const handleToggleAccountMute = async (userId: number, muted: boolean) => {
    setBusySessions((prev) => ({ ...prev, [userId]: "mute" }));
    try {
      await invoke<boolean>("toggle_account_mute", { userId, muted });
      setMutedAccounts((prev) => ({ ...prev, [userId]: muted }));
    } catch (err) {
      appendLog("error", `Mute toggle failed: ${String(err)}`);
    } finally {
      setBusySessions((prev) => ({ ...prev, [userId]: undefined }));
    }
  };

  // Favorites
  const handleToggleFavorite = async (game: GameSearchResult) => {
    const isFav = appConfig.favorites.some((f) => f.placeId === game.placeId);
    try {
      if (isFav) {
        const nextCfg = await invoke<AppConfig>("remove_favorite_game", {
          placeId: game.placeId,
        });
        setAppConfig(nextCfg);
        appendLog("info", `Removed "${game.name}" from Favorites.`);
      } else {
        const vip =
          privateServerLink.trim() && vipValidation.valid ? privateServerLink.trim() : null;
        const favorite: FavoriteGame = {
          placeId: game.placeId,
          universeId: game.universeId,
          name: game.name,
          iconUrl: game.iconUrl,
          privateServerLink: vip,
        };
        const nextCfg = await invoke<AppConfig>("save_favorite_game", { favorite });
        setAppConfig(nextCfg);
        appendLog(
          "launched",
          `Saved "${game.name}"${vip ? " (with VIP link)" : ""} to Favorites.`,
        );
      }
    } catch (err) {
      appendLog("error", `Favorites update failed: ${String(err)}`);
    }
  };

  const handleSelectFavorite = (fav: FavoriteGame) => {
    setSelectedGame({
      universeId: fav.universeId,
      placeId: fav.placeId,
      name: fav.name,
      creatorName: "Favorite",
      playing: 0,
      visits: 0,
      maxPlayers: 0,
      iconUrl: fav.iconUrl,
    });
    setPrivateServerLink(fav.privateServerLink ?? "");
    if (fav.privateServerLink) {
      setJobId("");
    }
  };

  const handleRemoveFavorite = async (placeId: number) => {
    try {
      const nextCfg = await invoke<AppConfig>("remove_favorite_game", { placeId });
      setAppConfig(nextCfg);
    } catch (err) {
      appendLog("error", `Failed to remove favorite: ${String(err)}`);
    }
  };

  // Launch helper
  const executeLaunch = async (params: {
    userIds: number[];
    game: GameSearchResult;
    vipLink: string | null;
    targetJobId: string | null;
    sameServer: boolean;
    delay: number;
    autoArrange: boolean;
    monitorId: string | null;
    activeMuteMode: MuteMode;
  }) => {
    setIsLaunching(true);
    appendLog(
      "info",
      `Starting launch of ${params.userIds.length} account(s) into "${params.game.name}"${params.vipLink ? " (Private Server)" : params.sameServer ? " (Same Server)" : ""} with ${params.delay}s delay...`,
    );

    try {
      await invoke("launch_accounts", {
        userIds: params.userIds,
        placeId: params.game.placeId,
        jobId: params.vipLink ? null : params.targetJobId,
        privateServerLink: params.vipLink,
        delaySeconds: params.delay,
        autoArrange: params.autoArrange,
        joinSameServer: params.sameServer,
        monitorId: params.monitorId,
        muteMode: params.activeMuteMode,
      });
    } catch (err) {
      setIsLaunching(false);
      appendLog("error", `Failed to start launch: ${String(err)}`);
    }
  };

  const handleLaunch = async () => {
    if (!selectedGame || selectedIds.size === 0 || !vipValidation.valid) return;

    const orderedUserIds = accounts
      .filter((a) => selectedIds.has(a.userId))
      .map((a) => a.userId);
    const vipTrimmed = privateServerLink.trim() || null;
    const jobTrimmed = jobId.trim() || null;

    await executeLaunch({
      userIds: orderedUserIds,
      game: selectedGame,
      vipLink: vipTrimmed,
      targetJobId: jobTrimmed,
      sameServer: !vipTrimmed && !jobTrimmed && joinSameServer,
      delay: delaySeconds,
      autoArrange: rejoinConfig.autoArrange,
      monitorId: selectedMonitorId,
      activeMuteMode: muteMode,
    });
  };

  const handleCancelLaunch = async () => {
    try {
      await invoke("cancel_launch");
      appendLog("waiting", "Cancelling remaining launches...");
    } catch (err) {
      appendLog("error", `Failed to cancel launch: ${String(err)}`);
    }
  };

  // Profiles
  const handleSaveProfile = async (name: string) => {
    if (!selectedGame || selectedIds.size === 0) return;
    const orderedUserIds = accounts
      .filter((a) => selectedIds.has(a.userId))
      .map((a) => a.userId);

    const profile: LaunchProfile = {
      id: "",
      name,
      userIds: orderedUserIds,
      placeId: selectedGame.placeId,
      universeId: selectedGame.universeId,
      gameName: selectedGame.name,
      gameIconUrl: selectedGame.iconUrl,
      privateServerLink: privateServerLink.trim() || null,
      jobId: jobId.trim() || null,
      joinSameServer,
      delaySeconds,
      fpsCap: performance.fpsCap,
      lowCpuMode: performance.lowCpuMode,
      autoRejoinEnabled: rejoinConfig.enabled,
      autoRejoinIntervalMinutes: rejoinConfig.intervalMinutes,
      autoArrange: rejoinConfig.autoArrange,
      monitorId: selectedMonitorId,
      muteMode,
    };

    try {
      const nextCfg = await invoke<AppConfig>("save_launch_profile", { profile });
      setAppConfig(nextCfg);
      appendLog("launched", `Saved launch profile "${name}".`);
    } catch (err) {
      appendLog("error", `Failed to save profile: ${String(err)}`);
    }
  };

  const handleDeleteProfile = async (id: string) => {
    try {
      const nextCfg = await invoke<AppConfig>("delete_launch_profile", { id });
      setAppConfig(nextCfg);
    } catch (err) {
      appendLog("error", `Failed to delete profile: ${String(err)}`);
    }
  };

  const handleRunProfile = async (profile: LaunchProfile) => {
    const validAccountIds = new Set(accounts.map((a) => a.userId));
    const runnableIds = profile.userIds.filter((id) => validAccountIds.has(id));
    if (runnableIds.length === 0) {
      appendLog(
        "error",
        `Profile "${profile.name}" has no matching saved accounts. Add or update accounts first.`,
      );
      return;
    }

    const game: GameSearchResult = {
      universeId: profile.universeId ?? 0,
      placeId: profile.placeId,
      name: profile.gameName,
      creatorName: "Profile",
      playing: 0,
      visits: 0,
      maxPlayers: 0,
      iconUrl: profile.gameIconUrl,
    };

    // Load profile settings into UI state
    setSelectedIds(new Set(runnableIds));
    setSelectedGame(game);
    setPrivateServerLink(profile.privateServerLink ?? "");
    setJobId(profile.jobId ?? "");
    setJoinSameServer(profile.joinSameServer);
    setDelaySeconds(profile.delaySeconds);
    if (profile.monitorId) {
      setSelectedMonitorId(profile.monitorId);
    }
    setMuteMode(profile.muteMode);

    // Apply performance, rejoin, and mute settings before launching
    const nextPerf: PerformanceSettings = {
      fpsCap: profile.fpsCap,
      lowCpuMode: profile.lowCpuMode,
    };
    const nextRejoin: AutoRejoinConfig = {
      enabled: profile.autoRejoinEnabled,
      intervalMinutes: profile.autoRejoinIntervalMinutes,
      autoArrange: profile.autoArrange,
    };

    await handleApplyPerformance(nextPerf);
    await handleUpdateRejoinConfig(nextRejoin);
    await handleChangeMuteMode(profile.muteMode);

    await executeLaunch({
      userIds: runnableIds,
      game,
      vipLink: profile.privateServerLink,
      targetJobId: profile.jobId,
      sameServer:
        !profile.privateServerLink && !profile.jobId && profile.joinSameServer,
      delay: profile.delaySeconds,
      autoArrange: profile.autoArrange,
      monitorId: profile.monitorId ?? selectedMonitorId,
      activeMuteMode: profile.muteMode,
    });
  };

  // Auto Rejoin, Grid, Monitor, Audio & Performance handlers
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

  const handleSelectMonitor = async (monitorId: string | null) => {
    setSelectedMonitorId(monitorId);
    try {
      const nextCfg = await invoke<AppConfig>("update_app_settings", {
        settings: {
          ...appConfig.appSettings,
          defaultMonitorId: monitorId,
        },
      });
      setAppConfig(nextCfg);
    } catch {
      // Non-fatal
    }
  };

  const handleArrangeWindows = async () => {
    setArranging(true);
    try {
      const res = await invoke<ArrangeWindowsResult>("arrange_windows", {
        monitorId: selectedMonitorId,
      });
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

  const handleChangeMuteMode = async (mode: MuteMode) => {
    setMuteMode(mode);
    setApplyingMute(true);
    try {
      const res = await invoke<AudioApplyResult>("apply_audio_mute_mode", { mode });
      const label =
        mode === "off"
          ? "Normal Sound"
          : mode === "mute_all"
            ? "Mute All"
            : "Mute Alts (Keep 1st)";
      appendLog(
        "info",
        `Audio mode set to ${label} (${res.mutedCount} muted, ${res.unmutedCount} unmuted).`,
      );
    } catch (err) {
      appendLog("error", `Audio mute error: ${String(err)}`);
    } finally {
      setApplyingMute(false);
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

  // App Settings Modal handlers
  const handleSaveSettings = async (next: AppSettings) => {
    const updated = await invoke<AppConfig>("update_app_settings", {
      settings: {
        ...next,
        defaultMonitorId: selectedMonitorId,
        muteMode,
      },
    });
    setAppConfig(updated);
    appendLog("launched", "RoMulti settings saved.");
  };

  const handleTestWebhook = async (url: string) => {
    await invoke("test_discord_webhook", { url });
  };

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-zinc-950 text-zinc-100">
      <TopBar
        mutexStatus={mutexStatus}
        onRefreshMutex={loadMutexStatus}
        onReacquireMutex={handleReacquireMutex}
        onOpenSettings={() => setSettingsOpen(true)}
        reacquiring={reacquiring}
      />

      <ProfilesBar
        profiles={appConfig.profiles}
        canSaveCurrent={selectedGame !== null && selectedIds.size > 0 && vipValidation.valid}
        disabled={isLaunching}
        onSaveProfile={handleSaveProfile}
        onRunProfile={handleRunProfile}
        onDeleteProfile={handleDeleteProfile}
      />

      <main className="flex min-h-0 flex-1 overflow-hidden">
        <AccountsPanel
          accounts={accounts}
          selectedIds={selectedIds}
          verifyStates={verifyStates}
          sessions={sessions}
          mutedAccounts={mutedAccounts}
          busySessions={busySessions}
          isAuthenticating={isAuthenticating}
          authMessage={authMessage}
          authError={authError}
          disabled={isLaunching}
          onAddAccount={handleAddAccount}
          onToggleSelect={handleToggleSelect}
          onSelectAll={handleSelectAll}
          onSelectNone={handleSelectNone}
          onSelectByTag={handleSelectByTag}
          onUpdateTags={handleUpdateTags}
          onRemoveAccount={handleRemoveAccount}
          onVerifyAccount={handleVerifyAccount}
          onFocusAccount={handleFocusAccount}
          onRejoinAccount={handleRejoinAccount}
          onKillAccount={handleKillAccount}
          onToggleAccountMute={handleToggleAccountMute}
          onDismissAuthError={() => setAuthError(null)}
        />

        <GameSearchPanel
          query={query}
          jobId={jobId}
          privateServerLink={privateServerLink}
          joinSameServer={joinSameServer}
          selectedAccountCount={selectedIds.size}
          vipValidation={vipValidation}
          results={results}
          favorites={appConfig.favorites}
          selectedGame={selectedGame}
          searching={searching}
          searchError={searchError}
          hasSearched={hasSearched}
          onQueryChange={setQuery}
          onJobIdChange={setJobId}
          onPrivateServerLinkChange={setPrivateServerLink}
          onJoinSameServerChange={setJoinSameServer}
          onSearchSubmit={() => executeSearch(query)}
          onSelectGame={setSelectedGame}
          onToggleFavorite={handleToggleFavorite}
          onSelectFavorite={handleSelectFavorite}
          onRemoveFavorite={handleRemoveFavorite}
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
        monitors={monitors}
        selectedMonitorId={selectedMonitorId}
        muteMode={muteMode}
        applyingPerf={applyingPerf}
        arranging={arranging}
        applyingMute={applyingMute}
        onDelayChange={setDelaySeconds}
        onLaunch={handleLaunch}
        onCancelLaunch={handleCancelLaunch}
        onClearLogs={() => setLogs([])}
        onUpdateRejoinConfig={handleUpdateRejoinConfig}
        onStopAutoRejoin={handleStopAutoRejoin}
        onArrangeWindows={handleArrangeWindows}
        onSelectMonitor={handleSelectMonitor}
        onChangeMuteMode={handleChangeMuteMode}
        onApplyPerformance={handleApplyPerformance}
      />

      <SettingsModal
        open={settingsOpen}
        settings={appConfig.appSettings}
        onClose={() => setSettingsOpen(false)}
        onSave={handleSaveSettings}
        onTestWebhook={handleTestWebhook}
      />
    </div>
  );
}

export default App;
