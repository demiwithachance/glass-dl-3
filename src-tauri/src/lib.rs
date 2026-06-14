use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus { yt_dlp: bool, ffmpeg: bool, deno: bool }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoInfo {
    title: String,
    uploader: Option<String>,
    thumbnail: Option<String>,
    duration_text: Option<String>,
    qualities: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadRequest {
    url: String,
    output_format: String,
    quality: String,
    folder: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent { percent: f64, speed: String, eta: String }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompleteEvent { path: String, file_name: String, folder: String }

#[derive(Clone, Serialize)]
struct ErrorEvent { message: String }

fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn tool_candidates(app: &AppHandle, name: &str) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    let resource_dir = app.path().resource_dir().ok();
    let cwd = std::env::current_dir().ok();
    let mut paths = Vec::new();
    for base in [exe_dir, resource_dir, cwd].into_iter().flatten() {
        paths.push(base.join("tools").join(name));
        paths.push(base.join(name));
        paths.push(base.join("src-tauri").join("binaries").join(name));
    }
    paths
}

fn find_tool(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    tool_candidates(app, name).into_iter().find(|p| p.is_file()).ok_or_else(|| {
        format!("{name} was not found. Put it in a 'tools' folder beside Glass DL.exe, then reopen the app.")
    })
}

fn default_downloads() -> Result<PathBuf, String> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from).map(|p| p.join("Downloads"))
        .ok_or_else(|| "Could not find your Downloads folder.".to_string())
}

#[tauri::command]
fn tool_status(app: AppHandle) -> ToolStatus {
    ToolStatus {
        yt_dlp: find_tool(&app, "yt-dlp.exe").is_ok(),
        ffmpeg: find_tool(&app, "ffmpeg.exe").is_ok(),
        deno: find_tool(&app, "deno.exe").is_ok(),
    }
}

#[tauri::command]
async fn fetch_video_info(app: AppHandle, url: String) -> Result<VideoInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let yt_dlp = find_tool(&app, "yt-dlp.exe")?;
        let mut command = Command::new(yt_dlp);
        hidden(&mut command).args(["--dump-single-json", "--no-playlist", "--no-warnings"]);
        if let Ok(deno) = find_tool(&app, "deno.exe") {
            let runtime = format!("deno:{}", deno.to_string_lossy());
            command.args(["--js-runtimes", runtime.as_str()]);
        }
        let output = command.args(["--", &url]).output().map_err(|e| format!("Could not start yt-dlp: {e}"))?;
        if !output.status.success() {
            let text = String::from_utf8_lossy(&output.stderr);
            return Err(readable_error(&text));
        }
        let data: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("yt-dlp returned unreadable video info: {e}"))?;
        let mut heights: Vec<u64> = data["formats"].as_array().into_iter().flatten()
            .filter(|f| f["vcodec"].as_str().unwrap_or("none") != "none")
            .filter_map(|f| f["height"].as_u64()).collect();
        heights.sort_unstable();
        heights.dedup();
        let targets = [2160_u64, 1440, 1080, 720, 480, 360];
        let mut qualities = vec!["best".to_string()];
        qualities.extend(targets.into_iter().filter(|h| heights.contains(h)).map(|h| h.to_string()));
        let duration_text = data["duration"].as_f64().map(|seconds| {
            let total = seconds.round() as u64;
            format!("{}:{:02}", total / 60, total % 60)
        });
        Ok(VideoInfo {
            title: data["title"].as_str().unwrap_or("Untitled video").to_string(),
            uploader: data["uploader"].as_str().map(str::to_string),
            thumbnail: data["thumbnail"].as_str().map(str::to_string),
            duration_text,
            qualities,
        })
    }).await.map_err(|e| format!("Video check task failed: {e}"))?
}

fn readable_error(raw: &str) -> String {
    let cleaned = raw.lines().filter(|line| !line.trim().is_empty()).last().unwrap_or(raw).trim();
    cleaned.strip_prefix("ERROR: ").unwrap_or(cleaned).to_string()
}

fn download_args(request: &DownloadRequest, ffmpeg: Option<&Path>, deno: Option<&Path>, output: &Path) -> Vec<String> {
    let mut args = vec![
        "--newline".into(), "--no-playlist".into(),
        "--progress-template".into(), "download:GLASS_PROGRESS|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s".into(),
        "--print".into(), "after_move:GLASS_COMPLETE|%(filepath)s".into(),
        "-o".into(), output.join("%(title).180B [%(id)s].%(ext)s").to_string_lossy().into_owned(),
    ];
    if let Some(path) = ffmpeg.and_then(Path::parent) {
        args.extend(["--ffmpeg-location".into(), path.to_string_lossy().into_owned()]);
    }
    if let Some(path) = deno {
        args.extend(["--js-runtimes".into(), format!("deno:{}", path.to_string_lossy())]);
    }
    if matches!(request.output_format.as_str(), "mp3" | "m4a" | "wav" | "ogg") {
        args.extend(["-x".into(), "--audio-format".into(), request.output_format.clone()]);
    } else {
        let selector = if request.quality == "best" {
            "bestvideo+bestaudio/best".to_string()
        } else {
            format!("bestvideo[height<={}]+bestaudio/best[height<={}]", request.quality, request.quality)
        };
        args.extend(["-f".into(), selector, "--merge-output-format".into(), request.output_format.clone()]);
    }
    args.extend(["--".into(), request.url.clone()]);
    args
}

#[tauri::command]
fn start_download(app: AppHandle, request: DownloadRequest) -> Result<(), String> {
    let yt_dlp = find_tool(&app, "yt-dlp.exe")?;
    let ffmpeg = find_tool(&app, "ffmpeg.exe").ok();
    let deno = find_tool(&app, "deno.exe").ok();
    let folder = request.folder.as_deref().map(PathBuf::from).map(Ok).unwrap_or_else(default_downloads)?;
    fs::create_dir_all(&folder).map_err(|e| format!("Could not use output folder: {e}"))?;
    let args = download_args(&request, ffmpeg.as_deref(), deno.as_deref(), &folder);
    tauri::async_runtime::spawn_blocking(move || {
        let mut command = Command::new(yt_dlp);
        hidden(&mut command).args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => return emit_error(&app, format!("Could not start yt-dlp: {e}")),
        };
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let error_app = app.clone();
        std::thread::spawn(move || {
            let mut last = String::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() { last = line; }
            }
            if !last.is_empty() && !last.contains("WARNING:") { let _ = error_app.emit("download-log-error", ErrorEvent { message: last }); }
        });
        let mut completed_path: Option<PathBuf> = None;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(data) = line.strip_prefix("GLASS_PROGRESS|") {
                let fields: Vec<&str> = data.split('|').collect();
                let percent = fields.first().unwrap_or(&"0").trim().trim_end_matches('%').parse().unwrap_or(0.0);
                let _ = app.emit("download-progress", ProgressEvent {
                    percent, speed: fields.get(1).unwrap_or(&"").trim().to_string(), eta: fields.get(2).unwrap_or(&"").trim().to_string(),
                });
            } else if let Some(path) = line.strip_prefix("GLASS_COMPLETE|") {
                completed_path = Some(PathBuf::from(path.trim()));
            }
        }
        match child.wait() {
            Ok(status) if status.success() => {
                let path = completed_path.unwrap_or_else(|| folder.clone());
                let _ = app.emit("download-complete", CompleteEvent {
                    file_name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                    path: path.to_string_lossy().into_owned(),
                    folder: folder.to_string_lossy().into_owned(),
                });
            }
            Ok(_) => emit_error(&app, "Download failed. Check the link, your connection, and yt-dlp version.".into()),
            Err(e) => emit_error(&app, format!("Could not finish the download process: {e}")),
        }
    });
    Ok(())
}

fn emit_error(app: &AppHandle, message: String) {
    let _ = app.emit("download-error", ErrorEvent { message });
}

#[tauri::command]
fn open_folder(folder: Option<String>) -> Result<(), String> {
    let path = folder.map(PathBuf::from).map(Ok).unwrap_or_else(default_downloads)?;
    let mut command = Command::new("explorer.exe");
    hidden(&mut command).arg(path).spawn().map_err(|e| format!("Could not open folder: {e}"))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![tool_status, fetch_video_info, start_download, open_folder])
        .run(tauri::generate_context!())
        .expect("error while running Glass DL");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DownloadRequest {
        DownloadRequest {
            url: "https://example.com/video".into(),
            output_format: "mp4".into(),
            quality: "best".into(),
            folder: None,
        }
    }

    #[test]
    fn download_arguments_do_not_require_deno() {
        let args = download_args(&request(), None, None, Path::new("downloads"));
        assert!(!args.iter().any(|arg| arg == "--js-runtimes"));
        assert!(args.iter().any(|arg| arg == "https://example.com/video"));
    }

    #[test]
    fn download_arguments_use_full_deno_path_when_available() {
        let deno = Path::new(r"C:\portable\tools\deno.exe");
        let args = download_args(&request(), None, Some(deno), Path::new("downloads"));
        let position = args.iter().position(|arg| arg == "--js-runtimes").unwrap();
        assert_eq!(args[position + 1], r"deno:C:\portable\tools\deno.exe");
    }
}
