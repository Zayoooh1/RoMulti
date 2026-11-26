import {
  AlertCircle,
  Check,
  CheckCircle2,
  Eye,
  Loader2,
  Plus,
  RefreshCw,
  RotateCw,
  Square,
  Tag,
  Trash2,
  User,
  Users,
  Volume2,
  VolumeX,
  X,
} from "lucide-react";
import { useMemo, useState } from "react";
import type { ActiveSessionInfo, PublicAccount } from "../types";

export type AccountVerifyState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "valid" }
  | { status: "invalid"; error: string };

interface AccountsPanelProps {
  accounts: PublicAccount[];
  selectedIds: Set<number>;
  verifyStates: Record<number, AccountVerifyState>;
  sessions: Record<number, ActiveSessionInfo>;
  mutedAccounts: Record<number, boolean>;
  busySessions: Record<number, string | undefined>;
  isAuthenticating: boolean;
  authMessage: string | null;
  authError: string | null;
  disabled: boolean;
  onAddAccount: () => void;
  onToggleSelect: (userId: number) => void;
  onSelectAll: () => void;
  onSelectNone: () => void;
  onSelectByTag: (tag: string) => void;
  onUpdateTags: (userId: number, tags: string[]) => Promise<void>;
  onRemoveAccount: (userId: number) => void;
  onVerifyAccount: (userId: number) => void;
  onFocusAccount: (userId: number) => void;
  onRejoinAccount: (userId: number) => void;
  onKillAccount: (userId: number) => void;
  onToggleAccountMute: (userId: number, muted: boolean) => void;
  onDismissAuthError: () => void;
}

function SessionStatusBadge({ session }: { session: ActiveSessionInfo }) {
  const title = session.pid
    ? `${session.lastMessage} (PID ${session.pid})`
    : session.lastMessage;

  switch (session.status) {
    case "launching":
      return (
        <span
          title={title}
          className="inline-flex shrink-0 items-center gap-1 rounded-full border border-sky-500/30 bg-sky-500/15 px-1.5 py-0.5 text-[10px] font-medium text-sky-300"
        >
          <Loader2 className="h-2.5 w-2.5 animate-spin" />
          Launching
        </span>
      );
    case "in_game":
      return (
        <span
          title={title}
          className="inline-flex shrink-0 items-center gap-1 rounded-full border border-emerald-500/30 bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-medium text-emerald-300"
        >
          <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
          In Game
        </span>
      );
    case "rejoining":
      return (
        <span
          title={title}
          className="inline-flex shrink-0 items-center gap-1 rounded-full border border-amber-500/30 bg-amber-500/15 px-1.5 py-0.5 text-[10px] font-medium text-amber-300"
        >
          <RotateCw className="h-2.5 w-2.5 animate-spin" />
          Rejoining...
        </span>
      );
    case "error":
      return (
        <span
          title={title}
          className="inline-flex shrink-0 items-center gap-1 rounded-full border border-red-500/30 bg-red-500/15 px-1.5 py-0.5 text-[10px] font-medium text-red-300"
        >
          Error
        </span>
      );
    default:
      return null;
  }
}

export function AccountsPanel({
  accounts,
  selectedIds,
  verifyStates,
  sessions,
  mutedAccounts,
  busySessions,
  isAuthenticating,
  authMessage,
  authError,
  disabled,
  onAddAccount,
  onToggleSelect,
  onSelectAll,
  onSelectNone,
  onSelectByTag,
  onUpdateTags,
  onRemoveAccount,
  onVerifyAccount,
  onFocusAccount,
  onRejoinAccount,
  onKillAccount,
  onToggleAccountMute,
  onDismissAuthError,
}: AccountsPanelProps) {
  const [editingTagsUserId, setEditingTagsUserId] = useState<number | null>(null);
  const [tagsDraft, setTagsDraft] = useState("");
  const [savingTags, setSavingTags] = useState(false);

  const selectedCount = selectedIds.size;

  const allTags = useMemo(() => {
    const counts = new Map<string, number>();
    for (const acc of accounts) {
      for (const t of acc.tags ?? []) {
        counts.set(t, (counts.get(t) ?? 0) + 1);
      }
    }
    return Array.from(counts.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  }, [accounts]);

  const startEditTags = (account: PublicAccount) => {
    setEditingTagsUserId(account.userId);
    setTagsDraft((account.tags ?? []).join(", "));
  };

  const handleSaveTags = async (userId: number) => {
    const parsed = tagsDraft
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    setSavingTags(true);
    try {
      await onUpdateTags(userId, parsed);
      setEditingTagsUserId(null);
    } finally {
      setSavingTags(false);
    }
  };

  return (
    <aside className="flex w-[360px] shrink-0 flex-col border-r border-zinc-800/80 bg-zinc-950/60">
      {/* Panel Header */}
      <div className="border-b border-zinc-800/80 p-4">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Users className="h-4 w-4 text-zinc-400" />
            <h2 className="text-sm font-semibold text-zinc-100">Accounts</h2>
            <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-[11px] font-medium text-zinc-400">
              {accounts.length}
            </span>
          </div>

          <button
            type="button"
            onClick={onAddAccount}
            disabled={disabled}
            className="flex items-center gap-1.5 rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-semibold text-white shadow-sm transition hover:bg-emerald-500 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isAuthenticating ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Plus className="h-3.5 w-3.5" />
            )}
            <span>Add Account</span>
          </button>
        </div>

        {/* Select All / None bar */}
        <div className="flex items-center justify-between text-xs">
          <span className="text-zinc-500">
            {selectedCount} of {accounts.length} selected
          </span>
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              onClick={onSelectAll}
              disabled={accounts.length === 0 || disabled}
              className="rounded px-2 py-0.5 text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-200 disabled:opacity-40"
            >
              Select All
            </button>
            <span className="text-zinc-700">/</span>
            <button
              type="button"
              onClick={onSelectNone}
              disabled={selectedCount === 0 || disabled}
              className="rounded px-2 py-0.5 text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-200 disabled:opacity-40"
            >
              None
            </button>
          </div>
        </div>

        {/* Tag Quick-Select Bar */}
        {allTags.length > 0 && (
          <div className="mt-2.5 flex flex-wrap items-center gap-1.5 border-t border-zinc-800/70 pt-2.5">
            <Tag className="h-3 w-3 shrink-0 text-zinc-500" />
            {allTags.map(([tag, count]) => (
              <button
                key={tag}
                type="button"
                disabled={disabled}
                onClick={() => onSelectByTag(tag)}
                title={`Select all accounts tagged "${tag}"`}
                className="inline-flex items-center gap-1 rounded-md border border-zinc-800 bg-zinc-900/80 px-2 py-0.5 text-[11px] font-medium text-zinc-300 transition hover:border-emerald-500/40 hover:bg-emerald-500/10 hover:text-emerald-300 disabled:opacity-45"
              >
                <span>{tag}</span>
                <span className="text-[10px] text-zinc-500">{count}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Auth Status / Error notifications */}
      {isAuthenticating && authMessage && (
        <div className="mx-3 mt-3 flex items-center gap-2 rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-200">
          <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-emerald-400" />
          <span className="truncate">{authMessage}</span>
        </div>
      )}

      {authError && (
        <div className="mx-3 mt-3 flex items-start justify-between gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
          <div className="flex items-start gap-2">
            <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-red-400" />
            <span className="leading-relaxed">{authError}</span>
          </div>
          <button
            type="button"
            onClick={onDismissAuthError}
            className="text-red-300 hover:text-white"
            aria-label="Dismiss error"
          >
            ×
          </button>
        </div>
      )}

      {/* Account List */}
      <div className="flex-1 overflow-y-auto p-3">
        {accounts.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center rounded-xl border border-dashed border-zinc-800 px-6 py-10 text-center">
            <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-full bg-zinc-900 text-zinc-500">
              <User className="h-6 w-6" />
            </div>
            <p className="text-sm font-medium text-zinc-300">No accounts added yet</p>
            <p className="mt-1 text-xs leading-relaxed text-zinc-500">
              Click <strong>Add Account</strong> to sign in securely via Roblox. Cookies are encrypted locally using Windows DPAPI.
            </p>
          </div>
        ) : (
          <ul className="space-y-2">
            {accounts.map((account) => {
              const isSelected = selectedIds.has(account.userId);
              const verify = verifyStates[account.userId] ?? { status: "idle" };
              const session = sessions[account.userId];
              const isMuted = mutedAccounts[account.userId] ?? false;
              const busyAction = busySessions[account.userId];
              const isEditingTags = editingTagsUserId === account.userId;
              const hasActiveSession =
                session &&
                (session.status === "in_game" ||
                  session.status === "error" ||
                  session.status === "rejoining");

              return (
                <li
                  key={account.userId}
                  onClick={() => !disabled && !isEditingTags && onToggleSelect(account.userId)}
                  className={`group rounded-xl border p-2.5 transition ${
                    isSelected
                      ? "border-emerald-500/40 bg-emerald-500/[0.08]"
                      : "border-zinc-800/80 bg-zinc-900/50 hover:border-zinc-700 hover:bg-zinc-900"
                  } ${disabled ? "opacity-80" : "cursor-pointer"}`}
                >
                  <div className="flex items-center gap-2.5">
                    {/* Custom Checkbox */}
                    <div
                      className={`flex h-4.5 w-4.5 shrink-0 items-center justify-center rounded border transition ${
                        isSelected
                          ? "border-emerald-500 bg-emerald-500 text-zinc-950"
                          : "border-zinc-600 bg-zinc-950 group-hover:border-zinc-400"
                      }`}
                    >
                      {isSelected && <Check className="h-3 w-3 stroke-[3]" />}
                    </div>

                    {/* Avatar */}
                    <div className="relative h-9 w-9 shrink-0 overflow-hidden rounded-full bg-zinc-800 ring-1 ring-zinc-700/80">
                      {account.avatarUrl ? (
                        <img
                          src={account.avatarUrl}
                          alt={account.username}
                          className="h-full w-full object-cover"
                          loading="lazy"
                        />
                      ) : (
                        <div className="flex h-full w-full items-center justify-center text-xs font-bold text-zinc-400">
                          {account.username.slice(0, 2).toUpperCase()}
                        </div>
                      )}
                    </div>

                    {/* Identity + Compact Session Status */}
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5">
                        <p className="truncate text-sm font-medium text-zinc-100">
                          {account.displayName || account.username}
                        </p>
                        {verify.status === "valid" && !session && (
                          <CheckCircle2
                            className="h-3.5 w-3.5 shrink-0 text-emerald-400"
                            aria-label="Session valid"
                          />
                        )}
                        {verify.status === "invalid" && (
                          <span
                            className="shrink-0 rounded bg-red-500/20 px-1.5 py-0.5 text-[10px] font-medium text-red-300"
                            title={verify.error}
                          >
                            Expired
                          </span>
                        )}
                      </div>
                      <div className="mt-0.5 flex items-center gap-1.5">
                        <p className="truncate text-xs text-zinc-500">@{account.username}</p>
                        {session && <SessionStatusBadge session={session} />}
                      </div>
                    </div>

                    {/* Row Actions */}
                    <div
                      className="flex shrink-0 items-center gap-0.5 opacity-70 transition group-hover:opacity-100"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <button
                        type="button"
                        onClick={() => startEditTags(account)}
                        disabled={disabled}
                        className="rounded-md p-1.5 text-zinc-400 transition hover:bg-zinc-800 hover:text-emerald-300 disabled:opacity-40"
                        title="Edit account tags (e.g. Main, Farm, Trade)"
                      >
                        <Tag className="h-3.5 w-3.5" />
                      </button>
                      <button
                        type="button"
                        onClick={() => onVerifyAccount(account.userId)}
                        disabled={verify.status === "checking" || disabled}
                        className="rounded-md p-1.5 text-zinc-400 transition hover:bg-zinc-800 hover:text-zinc-200 disabled:opacity-40"
                        title="Verify cookie & refresh profile"
                      >
                        <RefreshCw
                          className={`h-3.5 w-3.5 ${verify.status === "checking" ? "animate-spin text-emerald-400" : ""}`}
                        />
                      </button>
                      <button
                        type="button"
                        onClick={() => onRemoveAccount(account.userId)}
                        disabled={disabled}
                        className="rounded-md p-1.5 text-zinc-400 transition hover:bg-red-500/15 hover:text-red-400 disabled:opacity-40"
                        title="Remove account"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </div>
                  </div>

                  {/* Tags display or inline editor */}
                  {isEditingTags ? (
                    <form
                      onClick={(e) => e.stopPropagation()}
                      onSubmit={(e) => {
                        e.preventDefault();
                        handleSaveTags(account.userId);
                      }}
                      className="mt-2 flex items-center gap-1.5 pl-7"
                    >
                      <input
                        type="text"
                        autoFocus
                        value={tagsDraft}
                        onChange={(e) => setTagsDraft(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Escape") setEditingTagsUserId(null);
                        }}
                        placeholder="Main, Farm, Trade..."
                        className="min-w-0 flex-1 rounded-md border border-emerald-500/50 bg-zinc-950 px-2 py-1 text-[11px] text-zinc-100 placeholder-zinc-500 outline-none"
                      />
                      <button
                        type="submit"
                        disabled={savingTags}
                        className="rounded-md bg-emerald-600 px-2 py-1 text-[11px] font-medium text-white hover:bg-emerald-500 disabled:opacity-50"
                      >
                        {savingTags ? <Loader2 className="h-3 w-3 animate-spin" /> : "Save"}
                      </button>
                      <button
                        type="button"
                        onClick={() => setEditingTagsUserId(null)}
                        className="rounded-md p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    </form>
                  ) : (
                    account.tags &&
                    account.tags.length > 0 && (
                      <div className="mt-1.5 flex flex-wrap items-center gap-1 pl-7">
                        {account.tags.map((tag) => (
                          <span
                            key={tag}
                            className="rounded bg-zinc-800/90 px-1.5 py-0.5 text-[10px] font-medium text-zinc-400"
                          >
                            {tag}
                          </span>
                        ))}
                      </div>
                    )
                  )}

                  {/* Per-Instance Session Controls (Focus / Mute / Rejoin / Kill) */}
                  {hasActiveSession && (
                    <div
                      className="mt-2 flex items-center justify-between gap-2 border-t border-zinc-800/70 pt-2 pl-7"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <span className="truncate font-mono text-[10px] text-zinc-500">
                        {session.pid ? `PID ${session.pid}` : `Place ${session.placeId}`}
                      </span>

                      <div className="flex items-center gap-1">
                        <button
                          type="button"
                          disabled={!session.pid || Boolean(busyAction)}
                          onClick={() => onFocusAccount(account.userId)}
                          className="inline-flex items-center gap-1 rounded-md border border-zinc-800 bg-zinc-950/80 px-1.5 py-0.5 text-[10px] font-medium text-zinc-300 transition hover:border-emerald-500/40 hover:text-emerald-300 disabled:cursor-not-allowed disabled:opacity-40"
                          title="Bring this Roblox window to front"
                        >
                          {busyAction === "focus" ? (
                            <Loader2 className="h-2.5 w-2.5 animate-spin" />
                          ) : (
                            <Eye className="h-2.5 w-2.5" />
                          )}
                          <span>Focus</span>
                        </button>

                        <button
                          type="button"
                          disabled={!session.pid || Boolean(busyAction)}
                          onClick={() => onToggleAccountMute(account.userId, !isMuted)}
                          className={`inline-flex items-center justify-center rounded-md border px-1.5 py-0.5 text-[10px] transition disabled:cursor-not-allowed disabled:opacity-40 ${
                            isMuted
                              ? "border-amber-500/40 bg-amber-500/15 text-amber-300 hover:bg-amber-500/25"
                              : "border-zinc-800 bg-zinc-950/80 text-zinc-400 hover:border-zinc-700 hover:text-zinc-200"
                          }`}
                          title={isMuted ? "Unmute this Roblox instance" : "Mute this Roblox instance"}
                        >
                          {busyAction === "mute" ? (
                            <Loader2 className="h-2.5 w-2.5 animate-spin" />
                          ) : isMuted ? (
                            <VolumeX className="h-2.5 w-2.5" />
                          ) : (
                            <Volume2 className="h-2.5 w-2.5" />
                          )}
                        </button>

                        <button
                          type="button"
                          disabled={Boolean(busyAction)}
                          onClick={() => onRejoinAccount(account.userId)}
                          className="inline-flex items-center gap-1 rounded-md border border-zinc-800 bg-zinc-950/80 px-1.5 py-0.5 text-[10px] font-medium text-zinc-300 transition hover:border-amber-500/40 hover:text-amber-300 disabled:cursor-not-allowed disabled:opacity-40"
                          title="Restart & rejoin this account into its saved session"
                        >
                          {busyAction === "rejoin" ? (
                            <Loader2 className="h-2.5 w-2.5 animate-spin text-amber-400" />
                          ) : (
                            <RotateCw className="h-2.5 w-2.5" />
                          )}
                          <span>Rejoin</span>
                        </button>

                        <button
                          type="button"
                          disabled={Boolean(busyAction)}
                          onClick={() => onKillAccount(account.userId)}
                          className="inline-flex items-center gap-1 rounded-md border border-zinc-800 bg-zinc-950/80 px-1.5 py-0.5 text-[10px] font-medium text-zinc-300 transition hover:border-red-500/40 hover:bg-red-500/15 hover:text-red-300 disabled:cursor-not-allowed disabled:opacity-40"
                          title="Kill this Roblox process and stop Auto Rejoin for this account"
                        >
                          {busyAction === "kill" ? (
                            <Loader2 className="h-2.5 w-2.5 animate-spin text-red-400" />
                          ) : (
                            <Square className="h-2.5 w-2.5 fill-current" />
                          )}
                          <span>Kill</span>
                        </button>
                      </div>
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </aside>
  );
}
