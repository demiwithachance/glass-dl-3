import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

const FORMATS = ["mp4", "webm", "mp3", "m4a", "wav", "ogg"];
const VIDEO_FORMATS = new Set(["mp4", "webm"]);
const QUALITY_LABELS = {
  best: "Best",
  2160: "4K",
  1440: "1440p",
  1080: "1080p",
  720: "720p",
  480: "480p",
  360: "360p",
};

const settings = JSON.parse(localStorage.getItem("glass-dl-settings") || "{}");
const state = {
  info: null,
  folder: settings.lastFolder || "",
  format: settings.lastFormat || "mp4",
  quality: settings.lastQuality || "best",
  downloading: false,
};

document.documentElement.dataset.theme = settings.theme || "midnight";
document.querySelector("#app").innerHTML = `
  <div class="ambient ambient-one"></div><div class="ambient ambient-two"></div>
  <main class="shell">
    <header>
      <div class="brand"><div class="brand-mark">↓</div><div><strong>Glass DL</strong><span>quiet, quick downloads</span></div></div>
      <button class="icon-button" id="themeButton" title="Change glow theme">◐</button>
    </header>
    <nav class="tabs" aria-label="Services">
      <button class="tab active"><span class="service-dot youtube"></span>YouTube</button>
      <button class="tab future" title="Add another service tab in src/main.js">More services later</button>
    </nav>
    <section class="glass hero">
      <div class="section-heading"><div><span class="eyebrow">YouTube</span><h1>Bring a link. Pick the good bits.</h1></div><div class="tool-list" id="toolStatus"><span class="tool-pill">Checking tools…</span></div></div>
      <label class="url-box"><span>Video link</span><div><input id="urlInput" type="url" placeholder="https://www.youtube.com/watch?v=…" autocomplete="off"><button id="fetchButton">Fetch video</button></div></label>
      <div class="warning-message hidden" id="runtimeWarning">Some YouTube formats may be missing because no JavaScript runtime was found.</div>
      <div class="fetch-status hidden" id="fetchStatus"><div class="status-row"><span id="fetchText">Fetching video info…</span><span class="tiny-spinner"></span></div><div class="track"><i></i></div></div>
      <div class="error hidden" id="errorBox"></div>
    </section>
    <section class="glass video-card hidden" id="videoCard">
      <img id="thumbnail" alt="">
      <div class="video-copy"><span class="eyebrow">Ready to download</span><h2 id="videoTitle"></h2><p id="videoMeta"></p></div>
    </section>
    <section class="options-grid hidden" id="optionsArea">
      <div class="glass option-card"><span class="eyebrow">Output format</span><div class="choices" id="formatChoices"></div></div>
      <div class="glass option-card"><span class="eyebrow">Available quality</span><div class="choices" id="qualityChoices"></div></div>
      <div class="glass option-card folder-card"><span class="eyebrow">Save location</span><div class="folder-row"><div><strong id="folderName">Downloads</strong><span id="folderPath">Default Downloads folder</span></div><button class="secondary" id="folderButton">Choose</button></div></div>
    </section>
    <section class="glass download-panel hidden" id="downloadPanel">
      <div class="download-top"><div><span class="eyebrow" id="downloadEyebrow">Download</span><h2 id="downloadTitle">Ready when you are</h2></div><strong id="percent">0%</strong></div>
      <div class="progress-track"><i id="progressFill"></i><div class="loader-orb" id="loaderOrb"></div></div>
      <div class="stats"><span id="speed">Waiting</span><span id="eta">ETA —</span></div>
      <button class="download-button" id="downloadButton">Start download</button>
      <button class="secondary hidden" id="openFolderButton">Open folder</button>
    </section>
    <footer>Powered by yt-dlp · No artificial speed limits · VPN-friendly</footer>
  </main>`;

const $ = (id) => document.getElementById(id);
const show = (id, visible = true) => $(id).classList.toggle("hidden", !visible);
const error = (message = "") => {
  $("errorBox").textContent = message;
  show("errorBox", Boolean(message));
};
const saveSettings = () => {
  localStorage.setItem("glass-dl-settings", JSON.stringify({
    lastFolder: state.folder, lastFormat: state.format, lastQuality: state.quality,
    theme: document.documentElement.dataset.theme,
  }));
};
const buttonChoices = (target, values, selected, labeler, onPick) => {
  $(target).innerHTML = values.map((value) => `<button class="choice ${value === selected ? "selected" : ""}" data-value="${value}">${labeler(value)}</button>`).join("");
  $(target).querySelectorAll("button").forEach((button) => button.addEventListener("click", () => {
    $(target).querySelectorAll("button").forEach((item) => item.classList.remove("selected"));
    button.classList.add("selected");
    onPick(button.dataset.value);
  }));
};
const renderFormats = () => buttonChoices("formatChoices", FORMATS, state.format, (x) => x.toUpperCase(), (value) => {
  state.format = value;
  renderQualities();
  saveSettings();
});
const renderQualities = () => {
  const values = VIDEO_FORMATS.has(state.format) ? state.info.qualities : ["best"];
  if (!values.includes(state.quality)) state.quality = "best";
  buttonChoices("qualityChoices", values, state.quality, (x) => QUALITY_LABELS[x], (value) => {
    state.quality = value;
    saveSettings();
  });
};
const renderFolder = () => {
  const path = state.folder;
  $("folderName").textContent = path ? path.split(/[\\/]/).filter(Boolean).pop() : "Downloads";
  $("folderPath").textContent = path || "Default Downloads folder";
};
const setBusy = (busy) => {
  $("fetchButton").disabled = busy;
  $("urlInput").disabled = busy;
  show("fetchStatus", busy);
};

// Add loader.gif, loader.webp, or loader.png to assets/sprites to replace the CSS loader.
const loadOptionalSprite = async () => {
  try {
    const sprite = await invoke("optional_sprite");
    if (!sprite) return;
    const blob = new Blob([new Uint8Array(sprite.bytes)], { type: sprite.mimeType });
    $("loaderOrb").classList.add("custom-sprite");
    $("loaderOrb").style.backgroundImage = `url("${URL.createObjectURL(blob)}")`;
  } catch {
    // Keep the built-in loader when an optional sprite cannot be loaded.
  }
};

$("fetchButton").addEventListener("click", async () => {
  const url = $("urlInput").value.trim();
  if (!url) return error("Paste a YouTube link first.");
  error();
  setBusy(true);
  $("fetchText").textContent = "Fetching video info…";
  try {
    state.info = await invoke("fetch_video_info", { url });
    $("thumbnail").src = state.info.thumbnail || "";
    $("videoTitle").textContent = state.info.title;
    $("videoMeta").textContent = [state.info.uploader, state.info.durationText].filter(Boolean).join(" · ");
    renderFormats();
    renderQualities();
    renderFolder();
    show("videoCard");
    show("optionsArea");
    show("downloadPanel");
    $("downloadTitle").textContent = "Ready when you are";
  } catch (message) {
    error(String(message));
  } finally {
    setBusy(false);
  }
});
$("urlInput").addEventListener("keydown", (event) => {
  if (event.key === "Enter") $("fetchButton").click();
});
$("folderButton").addEventListener("click", async () => {
  const chosen = await open({ directory: true, multiple: false, defaultPath: state.folder || undefined });
  if (chosen) {
    state.folder = chosen;
    renderFolder();
    saveSettings();
  }
});
$("downloadButton").addEventListener("click", async () => {
  if (state.downloading) return;
  error();
  state.downloading = true;
  $("downloadButton").disabled = true;
  $("downloadButton").textContent = "Downloading…";
  show("openFolderButton", false);
  $("downloadTitle").textContent = state.info.title;
  $("downloadEyebrow").textContent = "Downloading";
  $("loaderOrb").classList.add("moving");
  try {
    await invoke("start_download", {
      request: { url: $("urlInput").value.trim(), outputFormat: state.format, quality: state.quality, folder: state.folder || null },
    });
  } catch (message) {
    state.downloading = false;
    $("downloadButton").disabled = false;
    $("downloadButton").textContent = "Try again";
    error(String(message));
  }
});
$("openFolderButton").addEventListener("click", () => invoke("open_folder", { folder: state.folder || null }));
$("themeButton").addEventListener("click", () => {
  document.documentElement.dataset.theme = document.documentElement.dataset.theme === "midnight" ? "aurora" : "midnight";
  saveSettings();
});

await listen("download-progress", ({ payload }) => {
  const percent = Math.max(0, Math.min(100, Number(payload.percent) || 0));
  $("percent").textContent = `${percent.toFixed(1)}%`;
  $("progressFill").style.width = `${percent}%`;
  $("speed").textContent = payload.speed || "Working…";
  $("eta").textContent = payload.eta ? `ETA ${payload.eta}` : "ETA —";
});
await listen("download-complete", ({ payload }) => {
  state.downloading = false;
  if (payload.folder) state.folder = payload.folder;
  saveSettings();
  $("percent").textContent = "100%";
  $("progressFill").style.width = "100%";
  $("downloadEyebrow").textContent = "Finished";
  $("downloadTitle").textContent = payload.fileName;
  $("speed").textContent = payload.path;
  $("eta").textContent = "Complete";
  $("downloadButton").disabled = false;
  $("downloadButton").textContent = "Download again";
  $("loaderOrb").classList.remove("moving");
  show("openFolderButton");
});
await listen("download-error", ({ payload }) => {
  state.downloading = false;
  $("downloadButton").disabled = false;
  $("downloadButton").textContent = "Try again";
  $("loaderOrb").classList.remove("moving");
  error(payload.message);
});

try {
  const status = await invoke("tool_status");
  const toolPill = (label, found, optional = false) =>
    `<span class="tool-pill ${found ? "" : optional ? "optional" : "missing"}">${label} ${found ? "found" : optional ? "optional · missing" : "missing"}</span>`;
  $("toolStatus").innerHTML = [
    toolPill("yt-dlp", status.ytDlp),
    toolPill("ffmpeg", status.ffmpeg),
    toolPill("Deno", status.deno, true),
  ].join("");
  // This warning belongs to the YouTube service only. Future service tabs should
  // decide independently whether a missing JavaScript runtime matters to them.
  show("runtimeWarning", !status.deno);
} catch {
  $("toolStatus").innerHTML = '<span class="tool-pill missing">Tool check unavailable</span>';
}
renderFolder();
loadOptionalSprite();
