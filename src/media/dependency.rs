use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecLocation {
    Portable(PathBuf),
    LocalApp(PathBuf),
    Preinstalled(PathBuf),
    Missing,
}

impl CodecLocation {
    pub fn is_found(&self) -> bool {
        !matches!(self, CodecLocation::Missing)
    }

    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            CodecLocation::Portable(p)
            | CodecLocation::LocalApp(p)
            | CodecLocation::Preinstalled(p) => Some(p),
            CodecLocation::Missing => None,
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            CodecLocation::Portable(p) => format!("Portable ({})", p.display()),
            CodecLocation::LocalApp(p) => format!("Local AppData ({})", p.display()),
            CodecLocation::Preinstalled(p) => format!("Preinstalled System ({})", p.display()),
            CodecLocation::Missing => "Not Found".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DependencyStatus {
    pub ffmpeg: CodecLocation,
    pub ffprobe: CodecLocation,
    pub ffmpeg_version: Option<String>,
}

impl DependencyStatus {
    pub fn is_ready(&self) -> bool {
        self.ffmpeg.is_found() && self.ffprobe.is_found()
    }

    pub fn is_preinstalled(&self) -> bool {
        matches!(self.ffmpeg, CodecLocation::Preinstalled(_))
            || matches!(self.ffprobe, CodecLocation::Preinstalled(_))
    }
}

/// Resolves the absolute path for a codec binary (e.g., "ffmpeg" or "ffprobe").
/// Search priority:
/// 1. Directory of current running executable (portable distribution)
/// 2. User AppData directory (%LOCALAPPDATA%\AirMedia\bin on Windows)
/// 3. System PATH directories (preinstalled codecs)
/// 4. Fallback to binary name
pub fn locate_binary(name: &str) -> (PathBuf, CodecLocation) {
    // 1. Check beside current executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = if cfg!(windows) {
                parent.join(format!("{name}.exe"))
            } else {
                parent.join(name)
            };
            if candidate.is_file() {
                return (candidate.clone(), CodecLocation::Portable(candidate));
            }
        }
    }

    // 2. Check local application directory
    #[cfg(windows)]
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let candidate = PathBuf::from(local_app_data)
            .join("AirMedia")
            .join("bin")
            .join(format!("{name}.exe"));
        if candidate.is_file() {
            return (candidate.clone(), CodecLocation::LocalApp(candidate));
        }
    }

    #[cfg(not(windows))]
    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("airmedia")
            .join("bin")
            .join(name);
        if candidate.is_file() {
            return (candidate.clone(), CodecLocation::LocalApp(candidate));
        }
    }

    // 3. Check system PATH
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = if cfg!(windows) {
                dir.join(format!("{name}.exe"))
            } else {
                dir.join(name)
            };
            if candidate.is_file() {
                return (candidate.clone(), CodecLocation::Preinstalled(candidate));
            }
        }
    }

    (PathBuf::from(name), CodecLocation::Missing)
}

pub fn resolve_binary_path(name: &str) -> PathBuf {
    let (path, _) = locate_binary(name);
    path
}

pub fn check_dependencies() -> DependencyStatus {
    let (ffmpeg_path, ffmpeg_loc) = locate_binary("ffmpeg");
    let (_, ffprobe_loc) = locate_binary("ffprobe");

    let mut version = None;
    if ffmpeg_loc.is_found() {
        let mut cmd = Command::new(&ffmpeg_path);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.arg("-version");

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let out_str = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = out_str.lines().next() {
                    version = Some(first_line.trim().to_string());
                }
            }
        }
    }

    DependencyStatus {
        ffmpeg: ffmpeg_loc,
        ffprobe: ffprobe_loc,
        ffmpeg_version: version,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadState {
    Idle,
    Downloading(String),
    Extracting,
    Completed(String),
    Failed(String),
}

#[derive(Clone)]
pub struct CodecDownloader {
    pub state: Arc<Mutex<DownloadState>>,
}

impl CodecDownloader {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DownloadState::Idle)),
        }
    }

    pub fn state(&self) -> DownloadState {
        self.state.lock().map(|s| s.clone()).unwrap_or(DownloadState::Idle)
    }

    pub fn is_busy(&self) -> bool {
        match self.state() {
            DownloadState::Downloading(_) | DownloadState::Extracting => true,
            _ => false,
        }
    }

    pub fn start_download(&self) {
        let state_arc = Arc::clone(&self.state);
        {
            let mut s = state_arc.lock().unwrap();
            *s = DownloadState::Downloading("Initializing download...".to_string());
        }

        thread::spawn(move || {
            let res = run_on_the_fly_installer(&state_arc);
            let mut s = state_arc.lock().unwrap();
            match res {
                Ok(msg) => *s = DownloadState::Completed(msg),
                Err(err) => *s = DownloadState::Failed(err),
            }
        });
    }
}

fn run_on_the_fly_installer(state: &Arc<Mutex<DownloadState>>) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let local_app_data = std::env::var("LOCALAPPDATA")
            .map_err(|_| "Failed to determine %LOCALAPPDATA% directory".to_string())?;
        let target_dir = PathBuf::from(&local_app_data).join("AirMedia").join("bin");
        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create directory {}: {e}", target_dir.display()))?;

        if let Ok(mut s) = state.lock() {
            *s = DownloadState::Downloading("Downloading official FFmpeg release essentials...".to_string());
        }

        let ps_script = format!(
            r#"
$ErrorActionPreference = 'Stop'
$destDir = '{dest}'
$zipPath = Join-Path $env:TEMP 'airmedia-ffmpeg.zip'
$extractDir = Join-Path $env:TEMP 'airmedia-ffmpeg-extract'

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
$url = 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip'

if (Get-Command curl.exe -ErrorAction SilentlyContinue) {{
    curl.exe -L -s -o $zipPath $url
}} else {{
    Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing
}}

if (Test-Path $extractDir) {{ Remove-Item -Recurse -Force $extractDir -ErrorAction SilentlyContinue }}
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

$ffmpeg = Get-ChildItem -Path $extractDir -Filter 'ffmpeg.exe' -Recurse | Select-Object -First 1
$ffprobe = Get-ChildItem -Path $extractDir -Filter 'ffprobe.exe' -Recurse | Select-Object -First 1

if (-not $ffmpeg -or -not $ffprobe) {{
    throw 'FFmpeg binaries not found inside downloaded package.'
}}

Copy-Item $ffmpeg.FullName (Join-Path $destDir 'ffmpeg.exe') -Force
Copy-Item $ffprobe.FullName (Join-Path $destDir 'ffprobe.exe') -Force

Remove-Item -Recurse -Force $extractDir -ErrorAction SilentlyContinue
Remove-Item -Force $zipPath -ErrorAction SilentlyContinue
"#,
            dest = target_dir.to_string_lossy().replace('\'', "''")
        );

        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script]);
        cmd.creation_flags(CREATE_NO_WINDOW);

        let output = cmd.output().map_err(|e| format!("Failed to run PowerShell downloader: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Download/Extraction failed: {stderr}"));
        }

        let ffmpeg_exe = target_dir.join("ffmpeg.exe");
        let ffprobe_exe = target_dir.join("ffprobe.exe");

        if ffmpeg_exe.is_file() && ffprobe_exe.is_file() {
            Ok(format!("Installed codecs into {}", target_dir.display()))
        } else {
            Err("Codecs could not be verified after installation.".to_string())
        }
    }

    #[cfg(not(windows))]
    {
        Err("On-the-fly automated download is currently configured for Windows. On Linux/macOS, please install ffmpeg via your package manager (e.g. apt install ffmpeg, brew install ffmpeg).".to_string())
    }
}
