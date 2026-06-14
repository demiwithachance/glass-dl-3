# Glass DL

A lightweight Windows desktop wrapper for yt-dlp with a dark glass interface. It fetches video metadata first, only shows available YouTube quality tiers, remembers your choices, and streams live progress without opening a command prompt.

## Folder structure

```text
glass-dl/
├─ assets/sprites/          optional custom sprite packs
├─ src/                     frontend UI
├─ src-tauri/               Rust/Tauri backend
│  └─ binaries/             optional development tool location
├─ tools/                   portable yt-dlp and ffmpeg location
├─ index.html
└─ package.json
```

## Tool setup

1. Download the current Windows `yt-dlp.exe` from the official yt-dlp releases page.
2. Download an FFmpeg Windows build and locate `ffmpeg.exe` and `ffprobe.exe`.
3. Optional: for the most reliable modern YouTube support, download `deno.exe` from the official Deno releases page. The app and non-YouTube services remain usable without it.
4. Put the files in the `tools` folder beside `Glass DL.exe`:

```text
Glass DL portable/
├─ Glass DL.exe
└─ tools/
   ├─ yt-dlp.exe
   ├─ ffmpeg.exe
   ├─ ffprobe.exe
   └─ deno.exe              recommended
```

When `deno.exe` exists, Glass DL automatically passes its full path to yt-dlp as `--js-runtimes deno:<full path>`. When it is absent, YouTube remains usable but the app warns that some YouTube formats may be missing. Deno is never required for the whole app.

For development, the same project-root `tools` folder works. The app never applies a download rate limit. It uses the normal network route available to yt-dlp, including an enabled VPN.

## Run in development

Install these one-time prerequisites:

- Node.js 20 or newer
- Rust stable with Cargo
- Microsoft C++ Build Tools and the WebView2 runtime (normally already present on Windows 10/11)

Then run:

```powershell
npm install
npm run tauri dev
```

## Build

```powershell
npm install
npm run tauri build
```

The installer builds appear under `src-tauri/target/release/bundle`. The standalone executable is `src-tauri/target/release/glass-dl.exe`. To make a portable folder, copy that executable, rename it to `Glass DL.exe`, and place the `tools` folder beside it. No installer is required to open that portable copy.

## Build on GitHub

The included `Build Windows Portable` GitHub Actions workflow compiles the app on a GitHub-hosted Windows computer, so your local PC does not need Rust or Node installed.

Open the repository's **Actions** tab, select **Build Windows Portable**, choose **Run workflow**, and download the `Glass-DL-Windows-Portable` artifact after it finishes. Add the tool executables listed above to the artifact's included `tools` folder.

## Adding services and sprites

The service tabs live near the top of `src/main.js`. Add a tab and a service-specific panel there, then add a matching Tauri command in `src-tauri/src/lib.rs`. Keep shared download progress events unchanged so every service can reuse the existing progress panel.

Custom sprite files belong in `assets/sprites`. Name the active loader `loader.gif`, `loader.webp`, or `loader.png`; the first one found is used automatically. Nothing Pokémon-related is bundled or hardcoded, and the fallback loader is pure CSS.

## Notes

- Audio conversion and separate video/audio stream merging require FFmpeg.
- Settings are stored locally by the WebView (`lastFolder`, format, quality, and theme).
- The default output location is the current user's Downloads folder.
- You are responsible for following the source site's terms and applicable copyright law.
