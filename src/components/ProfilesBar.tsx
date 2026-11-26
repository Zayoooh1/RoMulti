import { Bookmark, Check, Play, Plus, Trash2, X } from "lucide-react";
import { useState } from "react";
import type { LaunchProfile } from "../types";

interface ProfilesBarProps {
  profiles: LaunchProfile[];
  canSaveCurrent: boolean;
  disabled: boolean;
  onSaveProfile: (name: string) => Promise<void>;
  onRunProfile: (profile: LaunchProfile) => void;
  onDeleteProfile: (id: string) => void;
}

export function ProfilesBar({
  profiles,
  canSaveCurrent,
  disabled,
  onSaveProfile,
  onRunProfile,
  onDeleteProfile,
}: ProfilesBarProps) {
  const [creating, setCreating] = useState(false);
  const [profileName, setProfileName] = useState("");
  const [saving, setSaving] = useState(false);

  const handleConfirmSave = async () => {
    const trimmed = profileName.trim();
    if (!trimmed) return;
    setSaving(true);
    try {
      await onSaveProfile(trimmed);
      setProfileName("");
      setCreating(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-zinc-800/80 bg-zinc-900/40 px-5 py-1.5 text-xs">
      <div className="flex shrink-0 items-center gap-1.5 text-zinc-400">
        <Bookmark className="h-3.5 w-3.5 text-emerald-400" />
        <span className="font-medium text-zinc-300">Profiles</span>
      </div>

      {/* Saved Profiles list */}
      <div className="flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto py-0.5">
        {profiles.length === 0 && !creating ? (
          <span className="text-[11px] text-zinc-500">
            Save your accounts, game, VIP, FPS, Auto Rejoin &amp; monitor as a 1-click profile.
          </span>
        ) : (
          profiles.map((profile) => (
            <div
              key={profile.id}
              className="group inline-flex shrink-0 items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/80 pl-2 pr-1 py-0.5 transition hover:border-emerald-500/40"
              title={`${profile.gameName} • ${profile.userIds.length} account(s)${profile.privateServerLink ? " • VIP" : ""}${profile.joinSameServer ? " • Same Server" : ""}`}
            >
              <button
                type="button"
                disabled={disabled}
                onClick={() => onRunProfile(profile)}
                className="inline-flex items-center gap-1.5 py-0.5 text-left text-zinc-200 transition hover:text-emerald-300 disabled:opacity-45"
              >
                <Play className="h-2.5 w-2.5 fill-current text-emerald-400" />
                <span className="max-w-[140px] truncate font-medium">{profile.name}</span>
                <span className="rounded bg-zinc-800/90 px-1 text-[10px] text-zinc-400">
                  {profile.userIds.length}
                </span>
              </button>
              <button
                type="button"
                disabled={disabled}
                onClick={() => onDeleteProfile(profile.id)}
                className="rounded p-0.5 text-zinc-600 transition hover:bg-red-500/15 hover:text-red-400 disabled:opacity-40"
                title={`Delete profile "${profile.name}"`}
              >
                <Trash2 className="h-3 w-3" />
              </button>
            </div>
          ))
        )}
      </div>

      {/* Save as Profile trigger / inline form */}
      <div className="shrink-0">
        {creating ? (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              handleConfirmSave();
            }}
            className="flex items-center gap-1.5"
          >
            <input
              type="text"
              autoFocus
              value={profileName}
              onChange={(e) => setProfileName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setCreating(false);
                  setProfileName("");
                }
              }}
              placeholder="Profile name (e.g. AFK Farm)..."
              className="w-44 rounded-md border border-emerald-500/50 bg-zinc-950 px-2 py-1 text-xs text-zinc-100 placeholder-zinc-500 outline-none"
            />
            <button
              type="submit"
              disabled={saving || !profileName.trim()}
              className="inline-flex items-center gap-1 rounded-md bg-emerald-600 px-2 py-1 text-[11px] font-medium text-white transition hover:bg-emerald-500 disabled:opacity-45"
            >
              <Check className="h-3 w-3" />
              Save
            </button>
            <button
              type="button"
              onClick={() => {
                setCreating(false);
                setProfileName("");
              }}
              className="rounded-md p-1 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
              aria-label="Cancel profile creation"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </form>
        ) : (
          <button
            type="button"
            disabled={!canSaveCurrent || disabled}
            onClick={() => setCreating(true)}
            title={
              canSaveCurrent
                ? "Save current accounts, game, VIP, performance, rejoin, monitor & mute settings as a profile"
                : "Select a game and at least one account first"
            }
            className="inline-flex items-center gap-1.5 rounded-lg border border-zinc-700/80 bg-zinc-800/80 px-2.5 py-1 text-[11px] font-medium text-zinc-200 transition hover:border-emerald-500/40 hover:bg-zinc-800 hover:text-emerald-300 disabled:cursor-not-allowed disabled:opacity-45"
          >
            <Plus className="h-3 w-3 text-emerald-400" />
            <span>Save as Profile</span>
          </button>
        )}
      </div>
    </div>
  );
}
