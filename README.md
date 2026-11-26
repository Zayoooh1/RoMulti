# RoMulti

**RoMulti** is a lightweight Windows 11 desktop Multi-Instance Launcher and Account Manager for Roblox, built with **Rust**, **Tauri v2**, **React**, **TypeScript**, and **Tailwind CSS**.

## Features

- **Native Multi-Instance Support** — Automatically closes conflicting pre-existing Roblox processes and holds `ROBLOX_singletonEvent` & `ROBLOX_singletonMutex` via Win32 API for the entire app lifecycle.
- **Secure Account Manager (Windows DPAPI)** — Sign in through an isolated WebView2 login window (fresh session per account, automatic `.ROBLOSECURITY` capture and cookie cleanup). Cookies are encrypted at rest with Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`) in `%APPDATA%\RoMulti\accounts.json` and are never exposed to the frontend.
- **Game Search & Direct PlaceId / URL Lookup** — Search Roblox experiences by name with live active player counts and thumbnails, or paste a numeric `PlaceId` / Roblox game URL directly.
- **Private Server (VIP) & JobId Support** — Launch multiple accounts straight into a Private Server using either classic (`?privateServerLinkCode=...`) or modern share links (`https://www.roblox.com/share?code=...&type=Server`), or join a specific public server `JobId`.
- **Auto Rejoin (Minutes + Crash Recovery)** — Automatically rejoins accounts every `N` minutes (preventing 20-minute AFK kicks) while continuously monitoring process PIDs and Roblox Presence API to recover crashes or disconnects immediately, with a 30-second startup grace period and a 3-attempt failure safeguard.
- **Window Grid Arrangement & Multi-Monitor Support** — Evenly tiles all running Roblox windows across the desktop work area (excluding the taskbar) on your chosen display monitor, on demand or automatically after batch launch / Auto Rejoin.
- **FPS Cap & Low CPU Mode** — Writes `DFIntTaskSchedulerTargetFps` (`Default`, `15`, `30`, `60`, `240`) and optional lightweight FFlags to Roblox's `ClientAppSettings.json`, and lowers Roblox process priority to `BELOW_NORMAL_PRIORITY_CLASS` in Low CPU Mode.
- **Launch Profiles, Account Tags & Favorite Games/VIP** — Tag accounts (e.g. `Main`, `Farm`, `Trade`) for 1-click group selection, star favorite games together with their VIP links, and save complete launch presets (accounts, game, VIP, FPS, Low CPU, Auto Rejoin, monitor, mute mode) to `%APPDATA%\RoMulti\config.json` for 1-click launch.
- **Window Nickname Labeling & Per-Instance Controls** — Automatically renames each Roblox window title bar to `Roblox — @username` and provides per-account **Focus**, **Mute/Unmute**, **Rejoin**, and **Kill** controls directly in the accounts panel.
- **Public Server Browser & Join Same Server** — Browse live public servers with player count, ping, FPS, sorting (`Asc`/`Desc`), and minimum free-slot filtering, or enable **Same Server** to automatically capture account #1's server `JobId` and join all remaining accounts into it.
- **Auto-Mute Instances (Windows WASAPI)** — Mute all Roblox instances or mute alts while keeping the first account audible (`Normal` / `Mute All` / `Mute Alts (Keep 1st)`), plus individual per-account mute toggles.
- **System Tray & Discord Webhook Alerts** — Optionally minimize RoMulti to the Windows System Tray on close (with quick tray actions for Show/Hide, Arrange Windows, Mute All, Stop Auto Rejoin, and Quit) and receive Discord Webhook embeds for expired `.ROBLOSECURITY` cookies, paused Auto Rejoin retries, or optional rejoin notifications.

## Tech Stack

- **Backend:** Rust, Tauri v2, `windows` crate (Win32 API & DPAPI), `webview2-com`, `reqwest`, `tokio`
- **Frontend:** React 19, TypeScript, Tailwind CSS v4, Vite, `lucide-react`

## Prerequisites (Building from Source)

- Windows 10 / 11 (x64)
- [Node.js LTS](https://nodejs.org/)
- [Rust (`stable-x86_64-pc-windows-msvc`)](https://rustup.rs/)
- Visual Studio C++ Build Tools (MSVC)
- Microsoft Edge WebView2 Runtime (pre-installed on Windows 11)

## Development & Build

```bash
# Install frontend dependencies
npm install

# Run in development mode
npm run tauri dev

# Build production release (exe + installers)
npm run tauri build
```

## Security & Privacy

- Account session cookies (`.ROBLOSECURITY`) are stored **locally only** under `%APPDATA%\RoMulti\accounts.json`, encrypted with your current Windows user credentials via **DPAPI**.
- No telemetry, external servers, or third-party analytics — RoMulti communicates exclusively with official Roblox endpoints.

## License

MIT License — see [LICENSE](LICENSE) for details.
