import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  Check,
  CheckCircle2,
  Gamepad2,
  Globe,
  Hash,
  Link2,
  Loader2,
  RefreshCw,
  Search,
  Server,
  Star,
  Users,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  FavoriteGame,
  GameSearchResult,
  PublicServerInfo,
  PublicServersPage,
} from "../types";

export interface VipValidationResult {
  valid: boolean;
  kind?: "classic" | "share";
  error?: string;
}

export function validatePrivateServerLink(raw: string): VipValidationResult {
  const trimmed = raw.trim();
  if (!trimmed) return { valid: true };

  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return {
      valid: false,
      error: "Invalid URL format. Paste a roblox.com VIP or share link.",
    };
  }

  const host = parsed.hostname.toLowerCase();
  if (!host.endsWith("roblox.com")) {
    return { valid: false, error: "Link must belong to roblox.com." };
  }

  const params = new URLSearchParams(parsed.search);
  let privateCode = "";
  let shareCode = "";
  let shareType = "";

  for (const [k, v] of params.entries()) {
    const kl = k.toLowerCase();
    if (kl === "privateserverlinkcode") privateCode = v.trim();
    if (kl === "code") shareCode = v.trim();
    if (kl === "type") shareType = v.trim();
  }

  if (privateCode) {
    return { valid: true, kind: "classic" };
  }

  if (parsed.pathname.toLowerCase().startsWith("/share") && shareCode) {
    if (shareType && shareType.toLowerCase() !== "server") {
      return {
        valid: false,
        error: `Share link type "${shareType}" is not a Server link (expected type=Server).`,
      };
    }
    return { valid: true, kind: "share" };
  }

  return {
    valid: false,
    error: "Expected '?privateServerLinkCode=...' or 'roblox.com/share?code=...&type=Server'.",
  };
}

interface GameSearchPanelProps {
  query: string;
  jobId: string;
  privateServerLink: string;
  joinSameServer: boolean;
  selectedAccountCount: number;
  vipValidation: VipValidationResult;
  results: GameSearchResult[];
  favorites: FavoriteGame[];
  selectedGame: GameSearchResult | null;
  searching: boolean;
  searchError: string | null;
  hasSearched: boolean;
  onQueryChange: (value: string) => void;
  onJobIdChange: (value: string) => void;
  onPrivateServerLinkChange: (value: string) => void;
  onJoinSameServerChange: (value: boolean) => void;
  onSearchSubmit: () => void;
  onSelectGame: (game: GameSearchResult) => void;
  onToggleFavorite: (game: GameSearchResult) => void;
  onSelectFavorite: (fav: FavoriteGame) => void;
  onRemoveFavorite: (placeId: number) => void;
}

function formatPlayerCount(count: number): string {
  if (count >= 1_000_000) {
    return `${(count / 1_000_000).toFixed(1)}M`;
  }
  if (count >= 10_000) {
    return `${(count / 1_000).toFixed(1)}K`;
  }
  return count.toLocaleString();
}

export function GameSearchPanel({
  query,
  jobId,
  privateServerLink,
  joinSameServer,
  selectedAccountCount,
  vipValidation,
  results,
  favorites,
  selectedGame,
  searching,
  searchError,
  hasSearched,
  onQueryChange,
  onJobIdChange,
  onPrivateServerLinkChange,
  onJoinSameServerChange,
  onSearchSubmit,
  onSelectGame,
  onToggleFavorite,
  onSelectFavorite,
  onRemoveFavorite,
}: GameSearchPanelProps) {
  const hasVipInput = privateServerLink.trim().length > 0;
  const hasJobInput = jobId.trim().length > 0;
  const joinSameDisabled = hasVipInput || hasJobInput;

  const favoritePlaceIds = useMemo(
    () => new Set(favorites.map((f) => f.placeId)),
    [favorites],
  );

  // Server Browser state
  const [browserOpen, setBrowserOpen] = useState(false);
  const [sortOrder, setSortOrder] = useState<"Asc" | "Desc">("Asc");
  const [minFreeSlots, setMinFreeSlots] = useState<number>(Math.max(1, selectedAccountCount));
  const [servers, setServers] = useState<PublicServerInfo[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loadingServers, setLoadingServers] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);

  useEffect(() => {
    setMinFreeSlots(Math.max(1, selectedAccountCount));
  }, [selectedAccountCount]);

  const loadServers = useCallback(
    async (placeId: number, order: "Asc" | "Desc", cursor: string | null, append: boolean) => {
      setLoadingServers(true);
      setServerError(null);
      try {
        const page = await invoke<PublicServersPage>("get_public_servers", {
          placeId,
          sortOrder: order,
          cursor,
        });
        setServers((prev) => (append ? [...prev, ...page.servers] : page.servers));
        setNextCursor(page.nextPageCursor);
      } catch (err) {
        setServerError(String(err));
      } finally {
        setLoadingServers(false);
      }
    },
    [],
  );

  useEffect(() => {
    if (browserOpen && selectedGame) {
      loadServers(selectedGame.placeId, sortOrder, null, false);
    }
  }, [browserOpen, selectedGame, sortOrder, loadServers]);

  const filteredServers = useMemo(
    () =>
      servers.filter((s) => {
        const free = Math.max(0, s.maxPlayers - s.playing);
        return free >= minFreeSlots;
      }),
    [servers, minFreeSlots],
  );

  return (
    <section className="flex min-w-0 flex-1 flex-col bg-zinc-950">
      {/* Search Controls */}
      <div className="border-b border-zinc-800/80 p-4">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            setBrowserOpen(false);
            onSearchSubmit();
          }}
          className="flex gap-2.5"
        >
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-3.5 h-4 w-4 -translate-y-1/2 text-zinc-500" />
            <input
              type="text"
              value={query}
              onChange={(e) => {
                if (browserOpen) setBrowserOpen(false);
                onQueryChange(e.target.value);
              }}
              placeholder="Search Roblox game by name, PlaceId (e.g. 2753915549), or game URL..."
              className="w-full rounded-xl border border-zinc-800 bg-zinc-900/90 py-2.5 pr-9 pl-10 text-sm text-zinc-100 placeholder-zinc-500 transition outline-none focus:border-emerald-500/60 focus:ring-2 focus:ring-emerald-500/20"
            />
            {query && (
              <button
                type="button"
                onClick={() => onQueryChange("")}
                className="absolute top-1/2 right-2.5 -translate-y-1/2 rounded p-1 text-zinc-500 hover:text-zinc-300"
                aria-label="Clear search"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            )}
          </div>

          <button
            type="submit"
            disabled={searching || !query.trim()}
            className="flex shrink-0 items-center gap-2 rounded-xl border border-zinc-700 bg-zinc-800 px-4 py-2.5 text-sm font-medium text-zinc-100 transition hover:bg-zinc-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {searching ? (
              <Loader2 className="h-4 w-4 animate-spin text-emerald-400" />
            ) : (
              <Search className="h-4 w-4" />
            )}
            <span>Search</span>
          </button>

          {selectedGame && (
            <button
              type="button"
              onClick={() => setBrowserOpen((v) => !v)}
              className={`flex shrink-0 items-center gap-2 rounded-xl border px-3.5 py-2.5 text-sm font-medium transition ${
                browserOpen
                  ? "border-emerald-500/60 bg-emerald-500/15 text-emerald-300"
                  : "border-zinc-700 bg-zinc-900 text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"
              }`}
              title={`Browse public servers for ${selectedGame.name}`}
            >
              <Globe className="h-4 w-4" />
              <span className="hidden sm:inline">Servers</span>
            </button>
          )}
        </form>

        {/* Optional Private Server (VIP) Link + Server JobId + Join Same Server */}
        <div className="mt-2.5 grid grid-cols-1 gap-2 lg:grid-cols-12">
          <div className="lg:col-span-6">
            <div className="relative">
              <Link2
                className={`pointer-events-none absolute top-1/2 left-3 h-3.5 w-3.5 -translate-y-1/2 ${
                  hasVipInput
                    ? vipValidation.valid
                      ? "text-emerald-400"
                      : "text-red-400"
                    : "text-zinc-500"
                }`}
              />
              <input
                type="text"
                value={privateServerLink}
                onChange={(e) => onPrivateServerLinkChange(e.target.value)}
                placeholder="Private Server Link (optional) — ?privateServerLinkCode=... or /share?code=..."
                className={`w-full rounded-lg border bg-zinc-900/50 py-1.5 pr-20 pl-8 text-xs text-zinc-200 placeholder-zinc-600 transition outline-none focus:bg-zinc-900 ${
                  hasVipInput && !vipValidation.valid
                    ? "border-red-500/60 focus:border-red-500"
                    : hasVipInput && vipValidation.valid
                      ? "border-emerald-500/40 focus:border-emerald-500/70"
                      : "border-zinc-800/80 focus:border-zinc-700"
                }`}
              />
              {hasVipInput && (
                <div className="absolute top-1/2 right-2 flex -translate-y-1/2 items-center gap-1">
                  {vipValidation.valid ? (
                    <span className="inline-flex items-center gap-1 rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-medium text-emerald-300">
                      <CheckCircle2 className="h-2.5 w-2.5" />
                      VIP
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 rounded bg-red-500/15 px-1.5 py-0.5 text-[10px] font-medium text-red-300">
                      Invalid
                    </span>
                  )}
                  <button
                    type="button"
                    onClick={() => onPrivateServerLinkChange("")}
                    className="rounded p-0.5 text-zinc-500 hover:text-zinc-300"
                    aria-label="Clear private server link"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </div>
              )}
            </div>
            {hasVipInput && !vipValidation.valid && vipValidation.error && (
              <p className="mt-1 text-[11px] text-red-400">{vipValidation.error}</p>
            )}
          </div>

          <div className="flex items-center gap-2 lg:col-span-6">
            <div className="relative flex-1">
              <Server className="pointer-events-none absolute top-1/2 left-3 h-3.5 w-3.5 -translate-y-1/2 text-zinc-500" />
              <input
                type="text"
                value={jobId}
                disabled={hasVipInput}
                onChange={(e) => onJobIdChange(e.target.value)}
                placeholder={
                  hasVipInput
                    ? "JobId disabled when VIP link is set"
                    : "Optional Server JobId"
                }
                className="w-full rounded-lg border border-zinc-800/80 bg-zinc-900/50 py-1.5 pr-7 pl-8 text-xs text-zinc-200 placeholder-zinc-600 transition outline-none focus:border-zinc-700 focus:bg-zinc-900 disabled:cursor-not-allowed disabled:opacity-45"
              />
              {hasJobInput && !hasVipInput && (
                <button
                  type="button"
                  onClick={() => onJobIdChange("")}
                  className="absolute top-1/2 right-2 -translate-y-1/2 rounded p-0.5 text-zinc-500 hover:text-zinc-300"
                  aria-label="Clear JobId"
                >
                  <X className="h-3 w-3" />
                </button>
              )}
            </div>

            {/* Join Same Server toggle */}
            <label
              className={`flex shrink-0 items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[11px] font-medium select-none transition ${
                joinSameDisabled
                  ? "cursor-not-allowed border-zinc-800/60 bg-zinc-900/30 text-zinc-600"
                  : joinSameServer
                    ? "cursor-pointer border-emerald-500/40 bg-emerald-500/15 text-emerald-300"
                    : "cursor-pointer border-zinc-800 bg-zinc-900/60 text-zinc-400 hover:border-zinc-700 hover:text-zinc-200"
              }`}
              title={
                joinSameDisabled
                  ? "Disabled automatically when VIP link or manual JobId is set"
                  : "Launches account #1 first, captures its server JobId via Presence API, and joins all remaining accounts into the same server"
              }
            >
              <input
                type="checkbox"
                disabled={joinSameDisabled}
                checked={!joinSameDisabled && joinSameServer}
                onChange={(e) => onJoinSameServerChange(e.target.checked)}
                className="h-3 w-3 accent-emerald-500 disabled:opacity-40"
              />
              <span>Same Server</span>
            </label>

            {selectedGame && (
              <div
                className="hidden shrink-0 items-center gap-1 text-xs text-zinc-400 xl:flex"
                title={`Target PlaceId: ${selectedGame.placeId}`}
              >
                <Hash className="h-3.5 w-3.5 text-emerald-400" />
                <code className="rounded bg-zinc-900 px-1.5 py-0.5 font-mono text-[11px] text-emerald-300">
                  {selectedGame.placeId}
                </code>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Main Content Area: Server Browser OR Search Results / Favorites */}
      <div className="flex-1 overflow-y-auto p-4">
        {browserOpen && selectedGame ? (
          <div className="flex h-full flex-col">
            <div className="mb-3 flex flex-wrap items-center justify-between gap-2 rounded-xl border border-zinc-800/80 bg-zinc-900/50 px-3.5 py-2.5 text-xs">
              <div className="flex items-center gap-2">
                <Globe className="h-4 w-4 text-emerald-400" />
                <span className="font-semibold text-zinc-100">
                  Public Servers — {selectedGame.name}
                </span>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <select
                  value={sortOrder}
                  onChange={(e) => setSortOrder(e.target.value as "Asc" | "Desc")}
                  className="rounded-md border border-zinc-700 bg-zinc-950 px-2 py-1 text-xs text-zinc-200 outline-none focus:border-emerald-500/60"
                >
                  <option value="Asc">Fewest Players (Asc)</option>
                  <option value="Desc">Most Players (Desc)</option>
                </select>

                <label className="flex items-center gap-1.5 rounded-md border border-zinc-800 bg-zinc-950 px-2 py-1 text-zinc-400">
                  <span>Min free:</span>
                  <input
                    type="number"
                    min={0}
                    max={100}
                    value={minFreeSlots}
                    onChange={(e) => setMinFreeSlots(Math.max(0, Number(e.target.value) || 0))}
                    className="w-10 bg-transparent text-center font-mono text-zinc-100 outline-none"
                  />
                </label>

                <button
                  type="button"
                  disabled={loadingServers}
                  onClick={() => loadServers(selectedGame.placeId, sortOrder, null, false)}
                  className="inline-flex items-center gap-1 rounded-md border border-zinc-700 bg-zinc-800 px-2.5 py-1 text-zinc-200 transition hover:bg-zinc-700 disabled:opacity-50"
                >
                  <RefreshCw className={`h-3 w-3 ${loadingServers ? "animate-spin text-emerald-400" : ""}`} />
                  <span>Refresh</span>
                </button>

                <button
                  type="button"
                  onClick={() => setBrowserOpen(false)}
                  className="rounded-md p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
                  title="Close Server Browser"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
            </div>

            {serverError && (
              <div className="mb-3 flex items-center gap-2 rounded-xl border border-red-500/30 bg-red-500/10 px-3.5 py-2.5 text-xs text-red-200">
                <AlertCircle className="h-4 w-4 shrink-0 text-red-400" />
                <span>{serverError}</span>
              </div>
            )}

            {loadingServers && servers.length === 0 ? (
              <div className="flex flex-1 items-center justify-center text-xs text-zinc-400">
                <Loader2 className="mr-2 h-4 w-4 animate-spin text-emerald-400" />
                Loading public servers...
              </div>
            ) : filteredServers.length === 0 ? (
              <div className="flex flex-1 flex-col items-center justify-center rounded-xl border border-dashed border-zinc-800 p-8 text-center">
                <Server className="mb-2 h-8 w-8 text-zinc-700" />
                <p className="text-sm font-medium text-zinc-300">No matching servers found</p>
                <p className="mt-1 text-xs text-zinc-500">
                  Try lowering &quot;Min free&quot; slots or switching sort order.
                </p>
              </div>
            ) : (
              <div className="space-y-1.5">
                {filteredServers.map((srv) => {
                  const isChosen = jobId.trim() === srv.id && !hasVipInput;
                  const freeSlots = Math.max(0, srv.maxPlayers - srv.playing);
                  return (
                    <div
                      key={srv.id}
                      className={`flex items-center justify-between gap-3 rounded-xl border px-3.5 py-2 text-xs transition ${
                        isChosen
                          ? "border-emerald-500/50 bg-emerald-500/10"
                          : "border-zinc-800/80 bg-zinc-900/40 hover:border-zinc-700"
                      }`}
                    >
                      <div className="flex min-w-0 items-center gap-4">
                        <div className="flex items-center gap-1.5 font-medium text-zinc-200">
                          <Users className="h-3.5 w-3.5 text-emerald-400" />
                          <span>
                            {srv.playing} / {srv.maxPlayers}
                          </span>
                          <span className="rounded bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">
                            {freeSlots} free
                          </span>
                        </div>
                        {srv.ping != null && (
                          <span className="text-zinc-400">
                            Ping: <strong className="font-mono text-zinc-200">{srv.ping}ms</strong>
                          </span>
                        )}
                        {srv.fps != null && (
                          <span className="hidden text-zinc-400 sm:inline">
                            FPS:{" "}
                            <strong className="font-mono text-zinc-200">
                              {Math.round(srv.fps)}
                            </strong>
                          </span>
                        )}
                        <code className="hidden truncate font-mono text-[10px] text-zinc-500 xl:inline">
                          {srv.id.slice(0, 18)}…
                        </code>
                      </div>

                      <button
                        type="button"
                        onClick={() => {
                          if (hasVipInput) onPrivateServerLinkChange("");
                          onJobIdChange(srv.id);
                          setBrowserOpen(false);
                        }}
                        className={`shrink-0 rounded-lg px-3 py-1 text-xs font-medium transition ${
                          isChosen
                            ? "bg-emerald-500 text-zinc-950"
                            : "border border-zinc-700 bg-zinc-800 text-zinc-200 hover:border-emerald-500/50 hover:bg-emerald-600 hover:text-white"
                        }`}
                      >
                        {isChosen ? "Selected" : "Select Server"}
                      </button>
                    </div>
                  );
                })}

                {nextCursor && (
                  <div className="pt-2 text-center">
                    <button
                      type="button"
                      disabled={loadingServers}
                      onClick={() =>
                        loadServers(selectedGame.placeId, sortOrder, nextCursor, true)
                      }
                      className="inline-flex items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900 px-4 py-1.5 text-xs font-medium text-zinc-300 transition hover:border-zinc-700 hover:text-white disabled:opacity-50"
                    >
                      {loadingServers && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
                      <span>Load More Servers</span>
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>
        ) : (
          <>
            {searchError && (
              <div className="mb-4 flex items-center gap-2.5 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-xs text-red-200">
                <AlertCircle className="h-4 w-4 shrink-0 text-red-400" />
                <span>{searchError}</span>
              </div>
            )}

            {searching ? (
              <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
                {Array.from({ length: 8 }).map((_, i) => (
                  <div
                    key={i}
                    className="animate-pulse rounded-xl border border-zinc-800/70 bg-zinc-900/40 p-3"
                  >
                    <div className="aspect-square w-full rounded-lg bg-zinc-800/80" />
                    <div className="mt-3 h-4 w-3/4 rounded bg-zinc-800" />
                    <div className="mt-2 h-3 w-1/2 rounded bg-zinc-800/70" />
                  </div>
                ))}
              </div>
            ) : results.length > 0 ? (
              <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
                {results.map((game) => {
                  const isSelected = selectedGame?.placeId === game.placeId;
                  const isFav = favoritePlaceIds.has(game.placeId);
                  return (
                    <div
                      key={`${game.universeId}-${game.placeId}`}
                      role="button"
                      tabIndex={0}
                      onClick={() => onSelectGame(game)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          onSelectGame(game);
                        }
                      }}
                      className={`group relative flex cursor-pointer flex-col overflow-hidden rounded-xl border p-2.5 text-left transition ${
                        isSelected
                          ? "border-emerald-500 bg-emerald-500/[0.08] ring-2 ring-emerald-500/25"
                          : "border-zinc-800/80 bg-zinc-900/40 hover:border-zinc-700 hover:bg-zinc-900/80"
                      }`}
                    >
                      {/* Game Thumbnail */}
                      <div className="relative aspect-square w-full overflow-hidden rounded-lg bg-zinc-800">
                        {game.iconUrl ? (
                          <img
                            src={game.iconUrl}
                            alt={game.name}
                            className="h-full w-full object-cover transition duration-300 group-hover:scale-105"
                            loading="lazy"
                          />
                        ) : (
                          <div className="flex h-full w-full items-center justify-center text-zinc-600">
                            <Gamepad2 className="h-10 w-10" />
                          </div>
                        )}

                        {/* Favorite Star Button */}
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            onToggleFavorite(game);
                          }}
                          title={
                            isFav
                              ? "Remove from Favorites"
                              : hasVipInput && vipValidation.valid
                                ? "Save to Favorites with current VIP link"
                                : "Save to Favorites"
                          }
                          className={`absolute top-2 left-2 flex h-6 w-6 items-center justify-center rounded-full backdrop-blur-sm transition ${
                            isFav
                              ? "bg-amber-500/90 text-zinc-950 shadow"
                              : "bg-zinc-950/75 text-zinc-400 hover:text-amber-300"
                          }`}
                        >
                          <Star className={`h-3.5 w-3.5 ${isFav ? "fill-current" : ""}`} />
                        </button>

                        {/* Selected badge */}
                        {isSelected && (
                          <div className="absolute top-2 right-2 flex h-6 w-6 items-center justify-center rounded-full bg-emerald-500 text-zinc-950 shadow-md">
                            <Check className="h-3.5 w-3.5 stroke-[3]" />
                          </div>
                        )}

                        {/* Live Players Pill */}
                        <div className="absolute bottom-2 left-2 flex items-center gap-1.5 rounded-md bg-zinc-950/85 px-2 py-0.5 text-[11px] font-medium text-zinc-200 backdrop-blur-sm">
                          <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                          <Users className="h-3 w-3 text-zinc-400" />
                          <span>{formatPlayerCount(game.playing)}</span>
                        </div>
                      </div>

                      {/* Metadata */}
                      <div className="mt-2.5 min-w-0">
                        <h3
                          className="truncate text-sm font-semibold text-zinc-100"
                          title={game.name}
                        >
                          {game.name}
                        </h3>
                        <div className="mt-0.5 flex items-center justify-between gap-2 text-[11px] text-zinc-500">
                          <span className="truncate">by {game.creatorName}</span>
                          <span className="shrink-0 font-mono text-[10px] text-zinc-600">
                            {game.placeId}
                          </span>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : hasSearched ? (
              <div className="flex h-full flex-col items-center justify-center text-center">
                <Gamepad2 className="mb-3 h-10 w-10 text-zinc-700" />
                <p className="text-sm font-medium text-zinc-300">No experiences found</p>
                <p className="mt-1 max-w-sm text-xs text-zinc-500">
                  Try a different keyword or paste the numeric PlaceId directly from a Roblox game URL.
                </p>
              </div>
            ) : (
              <div className="flex h-full flex-col">
                {/* Favorites / VIP Section */}
                {favorites.length > 0 && (
                  <div className="mb-6">
                    <div className="mb-2.5 flex items-center gap-2">
                      <Star className="h-4 w-4 fill-amber-400 text-amber-400" />
                      <h3 className="text-xs font-semibold uppercase tracking-wider text-zinc-300">
                        Favorite Games &amp; VIP Servers
                      </h3>
                      <span className="rounded-full bg-zinc-800 px-1.5 py-0.5 text-[10px] text-zinc-400">
                        {favorites.length}
                      </span>
                    </div>
                    <div className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-4">
                      {favorites.map((fav) => {
                        const isSelected = selectedGame?.placeId === fav.placeId;
                        return (
                          <div
                            key={fav.placeId}
                            role="button"
                            tabIndex={0}
                            onClick={() => onSelectFavorite(fav)}
                            onKeyDown={(e) => {
                              if (e.key === "Enter" || e.key === " ") {
                                e.preventDefault();
                                onSelectFavorite(fav);
                              }
                            }}
                            className={`group relative flex cursor-pointer items-center gap-2.5 rounded-xl border p-2.5 text-left transition ${
                              isSelected
                                ? "border-emerald-500 bg-emerald-500/[0.08]"
                                : "border-zinc-800/80 bg-zinc-900/40 hover:border-zinc-700 hover:bg-zinc-900/80"
                            }`}
                          >
                            <div className="h-11 w-11 shrink-0 overflow-hidden rounded-lg bg-zinc-800">
                              {fav.iconUrl ? (
                                <img
                                  src={fav.iconUrl}
                                  alt={fav.name}
                                  className="h-full w-full object-cover"
                                  loading="lazy"
                                />
                              ) : (
                                <div className="flex h-full w-full items-center justify-center text-zinc-600">
                                  <Gamepad2 className="h-5 w-5" />
                                </div>
                              )}
                            </div>
                            <div className="min-w-0 flex-1">
                              <p className="truncate text-xs font-semibold text-zinc-100">
                                {fav.name}
                              </p>
                              <div className="mt-0.5 flex items-center gap-1.5">
                                <span className="font-mono text-[10px] text-zinc-500">
                                  {fav.placeId}
                                </span>
                                {fav.privateServerLink && (
                                  <span className="inline-flex items-center gap-0.5 rounded bg-emerald-500/15 px-1 py-0.5 text-[9px] font-medium text-emerald-300">
                                    <Link2 className="h-2.5 w-2.5" />
                                    VIP
                                  </span>
                                )}
                              </div>
                            </div>
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                onRemoveFavorite(fav.placeId);
                              }}
                              className="rounded p-1 text-zinc-600 opacity-0 transition group-hover:opacity-100 hover:bg-red-500/15 hover:text-red-400"
                              title="Remove from Favorites"
                            >
                              <X className="h-3.5 w-3.5" />
                            </button>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}

                <div className="flex flex-1 flex-col items-center justify-center text-center">
                  <div className="mb-3 flex h-14 w-14 items-center justify-center rounded-2xl bg-zinc-900 ring-1 ring-zinc-800">
                    <Gamepad2 className="h-7 w-7 text-zinc-500" />
                  </div>
                  <p className="text-sm font-medium text-zinc-200">Select a Roblox Experience</p>
                  <p className="mt-1 max-w-md text-xs leading-relaxed text-zinc-500">
                    Search any game by name or paste a <strong className="text-zinc-300">PlaceId</strong> above. Star a game card to save it (along with your VIP link) to Favorites.
                  </p>
                  <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
                    {["Blox Fruits", "Brookhaven", "Adopt Me", "2753915549"].map((preset) => (
                      <button
                        key={preset}
                        type="button"
                        onClick={() => onQueryChange(preset)}
                        className="rounded-lg border border-zinc-800 bg-zinc-900/70 px-3 py-1.5 text-xs text-zinc-400 transition hover:border-zinc-700 hover:text-zinc-200"
                      >
                        {preset}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </section>
  );
}
