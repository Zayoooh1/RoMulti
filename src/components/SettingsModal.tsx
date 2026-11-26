import { Bell, CheckCircle2, Loader2, Send, Settings, X, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppSettings } from "../types";

interface SettingsModalProps {
  open: boolean;
  settings: AppSettings;
  onClose: () => void;
  onSave: (next: AppSettings) => Promise<void>;
  onTestWebhook: (url: string) => Promise<void>;
}

export function SettingsModal({
  open,
  settings,
  onClose,
  onSave,
  onTestWebhook,
}: SettingsModalProps) {
  const [draft, setDraft] = useState<AppSettings>(settings);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);

  useEffect(() => {
    if (open) {
      setDraft(settings);
      setTestResult(null);
    }
  }, [open, settings]);

  if (!open) return null;

  const handleSave = async () => {
    setSaving(true);
    try {
      await onSave(draft);
      onClose();
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      await onTestWebhook(draft.discordWebhookUrl);
      setTestResult({ ok: true, message: "Test embed sent to Discord!" });
    } catch (err) {
      setTestResult({ ok: false, message: String(err) });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4 backdrop-blur-xs"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg rounded-2xl border border-zinc-800 bg-zinc-950 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-zinc-800/80 px-5 py-4">
          <div className="flex items-center gap-2.5">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-emerald-500/15 ring-1 ring-emerald-500/30">
              <Settings className="h-4 w-4 text-emerald-400" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-zinc-100">RoMulti Settings</h2>
              <p className="text-[11px] text-zinc-500">System Tray &amp; Discord Webhook Alerts</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-1.5 text-zinc-400 transition hover:bg-zinc-900 hover:text-zinc-200"
            aria-label="Close settings"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="space-y-5 px-5 py-4 text-xs">
          {/* Tray Section */}
          <div className="rounded-xl border border-zinc-800/80 bg-zinc-900/40 p-3.5">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm font-medium text-zinc-100">
                  Minimize to System Tray on close
                </p>
                <p className="mt-0.5 text-[11px] leading-relaxed text-zinc-500">
                  Closing the window hides RoMulti to the Windows notification tray so the multi-instance mutex and Auto Rejoin continue running in the background.
                </p>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={draft.minimizeToTray}
                onClick={() =>
                  setDraft((prev) => ({ ...prev, minimizeToTray: !prev.minimizeToTray }))
                }
                className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition ${
                  draft.minimizeToTray ? "bg-emerald-500" : "bg-zinc-700"
                }`}
              >
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition ${
                    draft.minimizeToTray ? "translate-x-4.5" : "translate-x-0.5"
                  }`}
                />
              </button>
            </div>
          </div>

          {/* Discord Webhook Section */}
          <div className="space-y-3 rounded-xl border border-zinc-800/80 bg-zinc-900/40 p-3.5">
            <div className="flex items-center gap-2">
              <Bell className="h-4 w-4 text-emerald-400" />
              <p className="text-sm font-medium text-zinc-100">Discord Webhook Notifications</p>
            </div>

            <div>
              <label className="mb-1 block text-[11px] font-medium text-zinc-400">
                Webhook URL
              </label>
              <div className="flex gap-2">
                <input
                  type="url"
                  value={draft.discordWebhookUrl}
                  onChange={(e) => {
                    setDraft((prev) => ({ ...prev, discordWebhookUrl: e.target.value }));
                    setTestResult(null);
                  }}
                  placeholder="https://discord.com/api/webhooks/..."
                  className="min-w-0 flex-1 rounded-lg border border-zinc-800 bg-zinc-950 px-3 py-2 text-xs text-zinc-100 placeholder-zinc-600 outline-none focus:border-emerald-500/60"
                />
                <button
                  type="button"
                  disabled={testing || !draft.discordWebhookUrl.trim()}
                  onClick={handleTest}
                  className="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-zinc-700 bg-zinc-800 px-3 py-2 text-xs font-medium text-zinc-200 transition hover:bg-zinc-700 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {testing ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin text-emerald-400" />
                  ) : (
                    <Send className="h-3.5 w-3.5 text-emerald-400" />
                  )}
                  <span>Test Webhook</span>
                </button>
              </div>
              {testResult && (
                <div
                  className={`mt-2 flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[11px] ${
                    testResult.ok
                      ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
                      : "border-red-500/30 bg-red-500/10 text-red-300"
                  }`}
                >
                  {testResult.ok ? (
                    <CheckCircle2 className="h-3.5 w-3.5 shrink-0" />
                  ) : (
                    <XCircle className="h-3.5 w-3.5 shrink-0" />
                  )}
                  <span>{testResult.message}</span>
                </div>
              )}
            </div>

            <div className="space-y-2 pt-1">
              <p className="text-[11px] font-medium text-zinc-400">Notify on events:</p>

              <label className="flex cursor-pointer items-center gap-2.5 text-zinc-200 select-none">
                <input
                  type="checkbox"
                  checked={draft.notifyExpiredCookie}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, notifyExpiredCookie: e.target.checked }))
                  }
                  className="h-3.5 w-3.5 rounded border-zinc-700 bg-zinc-950 accent-emerald-500"
                />
                <span>Expired or rejected .ROBLOSECURITY cookie</span>
              </label>

              <label className="flex cursor-pointer items-center gap-2.5 text-zinc-200 select-none">
                <input
                  type="checkbox"
                  checked={draft.notifyRejoinFailed}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, notifyRejoinFailed: e.target.checked }))
                  }
                  className="h-3.5 w-3.5 rounded border-zinc-700 bg-zinc-950 accent-emerald-500"
                />
                <span>Auto Rejoin paused after 3 failed attempts</span>
              </label>

              <label className="flex cursor-pointer items-center gap-2.5 text-zinc-300 select-none">
                <input
                  type="checkbox"
                  checked={draft.notifyEveryRejoin}
                  onChange={(e) =>
                    setDraft((prev) => ({ ...prev, notifyEveryRejoin: e.target.checked }))
                  }
                  className="h-3.5 w-3.5 rounded border-zinc-700 bg-zinc-950 accent-emerald-500"
                />
                <span>
                  Every crash / rejoin trigger{" "}
                  <span className="text-zinc-500">(optional, may send frequent alerts)</span>
                </span>
              </label>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-zinc-800/80 px-5 py-3.5">
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg border border-zinc-800 bg-zinc-900 px-3.5 py-1.5 text-xs font-medium text-zinc-300 transition hover:bg-zinc-800"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={saving}
            onClick={handleSave}
            className="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-4 py-1.5 text-xs font-semibold text-white transition hover:bg-emerald-500 disabled:opacity-50"
          >
            {saving && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            <span>Save Settings</span>
          </button>
        </div>
      </div>
    </div>
  );
}
