import {
  AlertCircle,
  Check,
  CheckCircle2,
  Gamepad2,
  Hash,
  Link2,
  Loader2,
  Search,
  Server,
  Users,
  X,
} from "lucide-react";
import type { GameSearchResult } from "../types";

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
  vipValidation: VipValidationResult;
  results: GameSearchResult[];
  selectedGame: GameSearchResult | null;
  searching: boolean;
  searchError: string | null;
  hasSearched: boolean;
  onQueryChange: (value: string) => void;
  onJobIdChange: (value: string) => void;
  onPrivateServerLinkChange: (value: string) => void;
  onSearchSubmit: () => void;
  onSelectGame: (game: GameSearchResult) => void;
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
  vipValidation,
  results,
  selectedGame,
  searching,
  searchError,
  hasSearched,
  onQueryChange,
  onJobIdChange,
  onPrivateServerLinkChange,
  onSearchSubmit,
  onSelectGame,
}: GameSearchPanelProps) {
  const hasVipInput = privateServerLink.trim().length > 0;

  return (
    <section className="flex min-w-0 flex-1 flex-col bg-zinc-950">
      {/* Search Controls */}
      <div className="border-b border-zinc-800/80 p-4">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            onSearchSubmit();
          }}
          className="flex gap-2.5"
        >
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-3.5 h-4 w-4 -translate-y-1/2 text-zinc-500" />
            <input
              type="text"
              value={query}
              onChange={(e) => onQueryChange(e.target.value)}
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
        </form>

        {/* Optional Private Server (VIP) Link + Server JobId */}
        <div className="mt-2.5 grid grid-cols-1 gap-2 lg:grid-cols-12">
          <div className="lg:col-span-7">
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
                placeholder="Private Server Link (optional) — ?privateServerLinkCode=... or /share?code=...&type=Server"
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

          <div className="flex items-center gap-2 lg:col-span-5">
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
                className="w-full rounded-lg border border-zinc-800/80 bg-zinc-900/50 py-1.5 pr-3 pl-8 text-xs text-zinc-200 placeholder-zinc-600 transition outline-none focus:border-zinc-700 focus:bg-zinc-900 disabled:cursor-not-allowed disabled:opacity-45"
              />
            </div>

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

      {/* Results Area */}
      <div className="flex-1 overflow-y-auto p-4">
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
              return (
                <button
                  key={`${game.universeId}-${game.placeId}`}
                  type="button"
                  onClick={() => onSelectGame(game)}
                  className={`group relative flex flex-col overflow-hidden rounded-xl border p-2.5 text-left transition ${
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
                </button>
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
          <div className="flex h-full flex-col items-center justify-center text-center">
            <div className="mb-3 flex h-14 w-14 items-center justify-center rounded-2xl bg-zinc-900 ring-1 ring-zinc-800">
              <Gamepad2 className="h-7 w-7 text-zinc-500" />
            </div>
            <p className="text-sm font-medium text-zinc-200">Select a Roblox Experience</p>
            <p className="mt-1 max-w-md text-xs leading-relaxed text-zinc-500">
              Search any game by name or paste a <strong className="text-zinc-300">PlaceId</strong> above. Click a game card to set it as the launch target for your selected accounts.
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
        )}
      </div>
    </section>
  );
}
