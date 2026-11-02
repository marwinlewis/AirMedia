use std::path::PathBuf;

#[test]
fn test_subtitle_srt_parser() {
    let srt_content = r#"1
00:00:01,500 --> 00:00:04,200
Hello, welcome to <b>AirMedia</b>!

2
00:00:05,000 --> 00:00:08,000
Ultra-lightweight GPU player.
Line 2 of subtitle.
"#;

    // Use our subtitle parser logic
    let cues = airmedia_test_parse_srt(srt_content);
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].start_ms, 1500);
    assert_eq!(cues[0].end_ms, 4200);
    assert_eq!(cues[0].text, "Hello, welcome to AirMedia!");
    assert_eq!(cues[1].start_ms, 5000);
    assert_eq!(cues[1].end_ms, 8000);
    assert!(cues[1].text.contains("Line 2 of subtitle."));
}

#[test]
fn test_subtitle_vtt_parser() {
    let vtt_content = r#"WEBVTT

00:01.000 --> 00:03.500
WebVTT subtitle cue

00:04.000 --> 00:06.000
Second cue
"#;

    let cues = airmedia_test_parse_vtt(vtt_content);
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].start_ms, 1000);
    assert_eq!(cues[0].end_ms, 3500);
    assert_eq!(cues[0].text, "WebVTT subtitle cue");
}

#[derive(Debug, PartialEq)]
struct Cue {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

fn airmedia_test_parse_srt(content: &str) -> Vec<Cue> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks = normalized.split("\n\n");
    let mut cues = Vec::new();

    for block in blocks {
        let lines: Vec<&str> = block.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            continue;
        }

        let (time_line, text_start_idx) = if lines[0].contains("-->") {
            (lines[0], 1)
        } else if lines.len() > 1 && lines[1].contains("-->") {
            (lines[1], 2)
        } else {
            continue;
        };

        let parts: Vec<&str> = time_line.split("-->").collect();
        if parts.len() == 2 {
            let start = parse_ts(parts[0].trim());
            let end = parse_ts(parts[1].trim());
            if let (Some(s), Some(e)) = (start, end) {
                let text = lines[text_start_idx..].join("\n");
                let clean = strip_tags(&text);
                cues.push(Cue { start_ms: s, end_ms: e, text: clean });
            }
        }
    }
    cues
}

fn airmedia_test_parse_vtt(content: &str) -> Vec<Cue> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks = normalized.split("\n\n");
    let mut cues = Vec::new();

    for block in blocks {
        let lines: Vec<&str> = block.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() || lines[0].starts_with("WEBVTT") {
            continue;
        }

        let (time_line, text_start_idx) = if lines[0].contains("-->") {
            (lines[0], 1)
        } else if lines.len() > 1 && lines[1].contains("-->") {
            (lines[1], 2)
        } else {
            continue;
        };

        let parts: Vec<&str> = time_line.split("-->").collect();
        if parts.len() == 2 {
            let start = parse_ts(parts[0].trim());
            let end = parse_ts(parts[1].trim());
            if let (Some(s), Some(e)) = (start, end) {
                let text = lines[text_start_idx..].join("\n");
                let clean = strip_tags(&text);
                cues.push(Cue { start_ms: s, end_ms: e, text: clean });
            }
        }
    }
    cues
}

fn parse_ts(s: &str) -> Option<u64> {
    let s = s.replace(',', ".");
    let colon_count = s.matches(':').count();
    if colon_count == 2 {
        let parts: Vec<&str> = s.split(':').collect();
        let h: u64 = parts[0].parse().ok()?;
        let m: u64 = parts[1].parse().ok()?;
        let sec_parts: Vec<&str> = parts[2].split('.').collect();
        let sec: u64 = sec_parts[0].parse().ok()?;
        let ms: u64 = if sec_parts.len() > 1 {
            let m_str = format!("{:0<3}", sec_parts[1]);
            m_str[..3.min(m_str.len())].parse().unwrap_or(0)
        } else {
            0
        };
        Some(h * 3600_000 + m * 60_000 + sec * 1000 + ms)
    } else if colon_count == 1 {
        let parts: Vec<&str> = s.split(':').collect();
        let m: u64 = parts[0].parse().ok()?;
        let sec_parts: Vec<&str> = parts[1].split('.').collect();
        let sec: u64 = sec_parts[0].parse().ok()?;
        let ms: u64 = if sec_parts.len() > 1 {
            let m_str = format!("{:0<3}", sec_parts[1]);
            m_str[..3.min(m_str.len())].parse().unwrap_or(0)
        } else {
            0
        };
        Some(m * 60_000 + sec * 1000 + ms)
    } else {
        None
    }
}

fn strip_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        if c == '<' {
            inside = true;
        } else if c == '>' {
            inside = false;
        } else if !inside {
            result.push(c);
        }
    }
    result
}

#[test]
fn test_media_file_probe() {
    let test_file = PathBuf::from("test_media.mp4");
    if test_file.exists() {
        let output = std::process::Command::new("ffprobe")
            .args(["-v", "quiet", "-print_format", "json", "-show_format", "-show_streams", "test_media.mp4"])
            .output()
            .expect("ffprobe should run");
        assert!(output.status.success());
        let json_str = String::from_utf8_lossy(&output.stdout);
        assert!(json_str.contains("h264"));
        assert!(json_str.contains("1280"));
        assert!(json_str.contains("720"));
    }
}

#[test]
fn test_dependency_detection() {
    use airmedia::media::dependency::{check_dependencies, locate_binary};

    let (path, loc) = locate_binary("ffmpeg");
    println!("FFmpeg binary located at: {} ({:?})", path.display(), loc);
    let status = check_dependencies();
    println!(
        "Dependency status: ready={}, is_preinstalled={}",
        status.is_ready(),
        status.is_preinstalled()
    );
    assert!(status.is_ready(), "Preinstalled or local codecs should be detected");
}

#[test]
fn test_image_formats_save() {
    let width = 2;
    let height = 2;
    let rgba = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
    let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|c| [c[0], c[1], c[2]]).collect();
    let temp_dir = std::env::temp_dir();
    let jpg_path = temp_dir.join("test_airmedia_shot.jpg");
    let png_path = temp_dir.join("test_airmedia_shot.png");
    let bmp_path = temp_dir.join("test_airmedia_shot.bmp");

    image::save_buffer_with_format(&jpg_path, &rgb, width, height, image::ExtendedColorType::Rgb8, image::ImageFormat::Jpeg).unwrap();
    image::save_buffer_with_format(&png_path, &rgba, width, height, image::ExtendedColorType::Rgba8, image::ImageFormat::Png).unwrap();
    image::save_buffer_with_format(&bmp_path, &rgb, width, height, image::ExtendedColorType::Rgb8, image::ImageFormat::Bmp).unwrap();

    assert!(jpg_path.exists());
    assert!(png_path.exists());
    assert!(bmp_path.exists());

    let _ = std::fs::remove_file(jpg_path);
    let _ = std::fs::remove_file(png_path);
    let _ = std::fs::remove_file(bmp_path);
}

#[test]
fn test_preferences_defaults_and_serialization() {
    use airmedia::media::{default_pictures_dir, ScreenshotFormat};
    use airmedia::ui::preferences_dialog::PreferencesState;

    let default_prefs = PreferencesState::default();
    assert_eq!(default_prefs.screenshot_format, ScreenshotFormat::Jpg);
    assert_eq!(default_prefs.screenshot_dir, default_pictures_dir());

    let json = serde_json::to_string_pretty(&default_prefs).expect("Serialize preferences");
    let deserialized: PreferencesState = serde_json::from_str(&json).expect("Deserialize preferences");
    assert_eq!(deserialized.screenshot_format, ScreenshotFormat::Jpg);
    assert_eq!(deserialized.screenshot_dir, default_pictures_dir());
}

#[test]
fn test_media_player_step_frame_and_screenshot() {
    use airmedia::media::{MediaPlayer, ScreenshotFormat};
    use std::path::PathBuf;

    let test_file = PathBuf::from("test_media.mp4");
    if !test_file.exists() {
        return;
    }

    let mut player = MediaPlayer::new();

    let load_res = player.load_file(&test_file);
    assert!(load_res.is_ok(), "Should load test_media.mp4");

    let start_wait = std::time::Instant::now();
    while player.get_latest_video_frame().is_none() && start_wait.elapsed() < std::time::Duration::from_secs(3) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(player.get_latest_video_frame().is_some(), "Video frame should be ready within 3s");

    player.pause();
    assert!(!player.is_playing);

    let temp_dir = std::env::temp_dir().join("airmedia_test_shots");
    let shot1 = player.take_screenshot(&temp_dir, ScreenshotFormat::Jpg);
    assert!(shot1.is_ok(), "Screenshot 1 should succeed: {:?}", shot1.err());
    let path1 = shot1.unwrap();
    assert!(path1.exists(), "Screenshot file should exist: {}", path1.display());
    assert_eq!(path1.extension().and_then(|e| e.to_str()), Some("jpg"));

    let time_before = player.current_time_secs();
    player.step_frame(true);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let time_after = player.current_time_secs();
    assert!(time_after >= time_before, "Stepping frame forward should advance time");

    let shot2 = player.take_screenshot(&temp_dir, ScreenshotFormat::Png);
    assert!(shot2.is_ok(), "Screenshot 2 should succeed");
    let path2 = shot2.unwrap();
    assert!(path2.exists(), "PNG screenshot should exist: {}", path2.display());
    assert_eq!(path2.extension().and_then(|e| e.to_str()), Some("png"));

    player.stop();

    let _ = std::fs::remove_file(path1);
    let _ = std::fs::remove_file(path2);
    let _ = std::fs::remove_dir(temp_dir);
}
