//! Tests for playlist navigation, playlist files, subtitle robustness, ffmpeg hardening,
//! filter builders and session persistence.

use std::path::{Path, PathBuf};

fn playlist_of(n: usize) -> airmedia::playlist::Playlist {
    let mut pl = airmedia::playlist::Playlist::new();
    for i in 0..n {
        pl.add(format!("track{i}.mp3"));
    }
    pl
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("airmedia_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// Playlist navigation, playlist files and natural sorting
// ---------------------------------------------------------------------------

#[test]
fn test_playlist_repeat_modes() {
    use airmedia::playlist::RepeatMode;

    let mut pl = playlist_of(3);
    pl.current_index = Some(2);
    assert!(pl.next().is_none(), "Repeat off stops at the end");

    pl.repeat_mode = RepeatMode::All;
    pl.current_index = Some(2);
    assert_eq!(pl.next().unwrap().title, "track0.mp3", "Repeat all wraps");
    pl.current_index = Some(0);
    assert_eq!(pl.prev().unwrap().title, "track2.mp3", "Repeat all wraps backwards");

    pl.repeat_mode = RepeatMode::One;
    pl.current_index = Some(1);
    assert_eq!(pl.next_auto().unwrap().title, "track1.mp3", "Auto-advance repeats the item");
    assert_eq!(pl.next().unwrap().title, "track2.mp3", "Manual Next still advances");
    assert_eq!(pl.prev().unwrap().title, "track1.mp3", "Manual Prev still goes back");
}

#[test]
fn test_playlist_shuffle_visits_each_item_once() {
    let mut pl = playlist_of(6);
    pl.is_shuffle = true;
    pl.current_index = Some(0);

    let mut seen = vec![0usize];
    while let Some(item) = pl.next() {
        let idx: usize = item.title.trim_start_matches("track").trim_end_matches(".mp3").parse().unwrap();
        assert!(!seen.contains(&idx), "shuffle repeated {idx} within one cycle");
        seen.push(idx);
    }
    assert_eq!(seen.len(), 6, "shuffle should visit every item before stopping");

    // Previous walks back through the actual play order.
    assert_eq!(pl.current_index, Some(seen[5]));
    pl.prev();
    assert_eq!(pl.current_index, Some(seen[4]));
}

#[test]
fn test_playlist_parse_m3u_and_pls() {
    use airmedia::playlist::parse_playlist;

    let base = Path::new("music");
    let m3u = "#EXTM3U\n#EXTINF:12,Song\nsong.mp3\n\nhttps://example.com/live.m3u8\n# comment\nsub/other.flac\n";
    let entries = parse_playlist(m3u, base, false);
    assert_eq!(
        entries,
        vec![
            base.join("song.mp3"),
            PathBuf::from("https://example.com/live.m3u8"),
            base.join("sub/other.flac"),
        ]
    );

    let pls = "[playlist]\nFile1=a.mp3\nTitle1=A\nfile2=http://radio.example/stream\nNumberOfEntries=2\n";
    let entries = parse_playlist(pls, base, true);
    assert_eq!(entries, vec![base.join("a.mp3"), PathBuf::from("http://radio.example/stream")]);
}

#[test]
fn test_playlist_m3u_round_trip_and_hls_detection() {
    use airmedia::playlist::Playlist;

    let dir = temp_dir("m3u");
    let a = dir.join("Episode 2.mkv");
    let b = dir.join("Episode 10.mkv");
    std::fs::write(&a, b"x").unwrap();
    std::fs::write(&b, b"x").unwrap();

    let mut pl = Playlist::new();
    pl.add(&a);
    pl.add(&b);
    pl.add("https://example.com/stream.m3u8");
    let list = dir.join("list.m3u8");
    pl.save_m3u(&list).unwrap();

    let collected = Playlist::collect_media(std::slice::from_ref(&list));
    assert_eq!(
        collected.media,
        vec![a.clone(), b.clone(), PathBuf::from("https://example.com/stream.m3u8")]
    );

    // A local HLS manifest is media, not a playlist to expand.
    let hls = dir.join("stream.m3u8");
    std::fs::write(&hls, "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg0.ts\n#EXT-X-ENDLIST\n").unwrap();
    assert_eq!(Playlist::collect_media(std::slice::from_ref(&hls)).media, vec![hls]);

    // Folders expand in natural order and skip non-media files.
    assert_eq!(Playlist::media_in_folder(&dir), vec![a, b]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_natural_sort_order() {
    use airmedia::playlist::natural_cmp;
    let mut names = vec!["ep10.mkv", "Ep2.mkv", "ep1.mkv", "ep02b.mkv"];
    names.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(names, vec!["ep1.mkv", "Ep2.mkv", "ep02b.mkv", "ep10.mkv"]);
}

// ---------------------------------------------------------------------------
// Subtitles: encodings, robustness, sidecar discovery
// ---------------------------------------------------------------------------

#[test]
fn test_subtitle_decoding_variants() {
    use airmedia::media::subtitle::decode_subtitle_bytes;

    assert_eq!(decode_subtitle_bytes(b"\xEF\xBB\xBFhello"), "hello");
    assert_eq!(decode_subtitle_bytes(b"caf\xE9"), "caf\u{e9}", "Latin-1 fallback");
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("h\u{e9}".encode_utf16().flat_map(|u| u.to_le_bytes()))
        .collect();
    assert_eq!(decode_subtitle_bytes(&utf16), "h\u{e9}");
}

#[test]
fn test_subtitle_parser_rejects_malformed_input_without_panicking() {
    use airmedia::media::subtitle::parse_srt;

    // Multi-byte characters in the millisecond field used to split a UTF-8 sequence.
    let cues = parse_srt("1\n00:00:01,5\u{e9} --> 00:00:02,000\nText\n");
    assert!(cues.len() <= 1);
    // Absurd hours overflow instead of wrapping.
    let cues = parse_srt("1\n99999999999999999:00:00,000 --> 99999999999999999:00:01,000\nText\n");
    assert!(cues.is_empty());
}

#[test]
fn test_sidecar_subtitles_and_latin1_file() {
    use airmedia::media::subtitle::{find_sidecar_subtitles, SubtitleTrack};

    let dir = temp_dir("sidecar");
    let media = dir.join("Movie.mkv");
    std::fs::write(&media, b"x").unwrap();
    std::fs::write(dir.join("Movie.en.srt"), b"1\n00:00:01,000 --> 00:00:02,000\nEnglish\n").unwrap();
    std::fs::write(dir.join("Movie.srt"), b"1\n00:00:01,000 --> 00:00:02,000\nCaf\xE9\n").unwrap();
    std::fs::write(dir.join("Other.srt"), b"").unwrap();

    let found = find_sidecar_subtitles(&media);
    assert_eq!(found, vec![dir.join("Movie.srt"), dir.join("Movie.en.srt")]);

    let track = SubtitleTrack::from_file(dir.join("Movie.srt")).expect("Latin-1 subtitle should load");
    assert_eq!(track.cues[0].text, "Caf\u{e9}");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// ffmpeg invocation hardening and metadata validation
// ---------------------------------------------------------------------------

#[test]
fn test_ffmpeg_input_args_whitelist_protocols() {
    use airmedia::media::{ffmpeg_input_args, is_url};

    assert!(is_url(Path::new("https://example.com/a.m3u8")));
    assert!(is_url(Path::new("rtsp://cam.local/stream")));
    assert!(!is_url(Path::new("C:\\Videos\\a.mp4")));
    assert!(!is_url(Path::new("/home/me/a.mp4")));
    assert!(!is_url(Path::new("concat:a.mp4|b.mp4")));

    let local = ffmpeg_input_args(Path::new("-evil.mp4"));
    assert_eq!(local[0], "-protocol_whitelist");
    assert_eq!(local[1], "file,crypto,data");
    assert_eq!(local[local.len() - 2], "-i");
    assert!(
        PathBuf::from(&local[local.len() - 1]).is_absolute(),
        "local inputs are made absolute so they never look like options"
    );

    let net = ffmpeg_input_args(Path::new("https://example.com/live.m3u8"));
    assert!(!net[1].split(',').any(|p| p == "file"), "network streams may not read local files");
    assert!(net.contains(&"-rw_timeout".to_string()));
    assert_eq!(net.last().unwrap(), "https://example.com/live.m3u8");
}

#[test]
fn test_media_info_dimension_limits_and_rotation() {
    use airmedia::media::probe::MediaInfo;

    let huge: MediaInfo = serde_json::from_str(
        r#"{"streams":[{"index":0,"codec_type":"video","width":65535,"height":65535}],"format":{}}"#,
    )
    .unwrap();
    assert!(huge.validate().is_err());
    assert_eq!(huge.video_dimensions(), None);

    let rotated: MediaInfo = serde_json::from_str(
        r#"{"streams":[{"index":0,"codec_type":"video","width":1920,"height":1080,
            "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]}],"format":{}}"#,
    )
    .unwrap();
    assert!(rotated.validate().is_ok());
    assert_eq!(rotated.video_dimensions(), Some((1080, 1920)));

    let empty: MediaInfo = serde_json::from_str(r#"{"streams":[],"format":{}}"#).unwrap();
    assert!(empty.validate().is_err());
}

#[test]
fn test_embedded_cover_art_is_not_video() {
    use airmedia::media::probe::MediaInfo;

    // An MP3 with album art: ffprobe lists the picture as a video stream.
    let song: MediaInfo = serde_json::from_str(
        r#"{"streams":[{"index":0,"codec_type":"audio","codec_name":"mp3"},
            {"index":1,"codec_type":"video","codec_name":"mjpeg","width":600,"height":600,
             "disposition":{"default":0,"attached_pic":1}}],"format":{}}"#,
    )
    .unwrap();
    assert!(song.validate().is_ok());
    assert!(song.primary_video_stream().is_none());
    assert_eq!(song.video_dimensions(), None);

    // Real video wins even when the cover comes first in the file.
    let movie: MediaInfo = serde_json::from_str(
        r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"png","width":600,"height":600,
             "disposition":{"attached_pic":1}},
            {"index":1,"codec_type":"video","codec_name":"h264","width":1280,"height":720,
             "disposition":{"attached_pic":0}},
            {"index":2,"codec_type":"audio","codec_name":"aac"}],"format":{}}"#,
    )
    .unwrap();
    assert_eq!(movie.video_dimensions(), Some((1280, 720)));

    // Motion JPEG is real video when it isn't flagged as a picture.
    let mjpeg: MediaInfo = serde_json::from_str(
        r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"mjpeg","width":640,"height":480,
             "disposition":{"attached_pic":0}},
            {"index":1,"codec_type":"audio","codec_name":"pcm_s16le"}],"format":{}}"#,
    )
    .unwrap();
    assert_eq!(mjpeg.video_dimensions(), Some((640, 480)));

    // A picture with no audio has nothing to play.
    let picture_only: MediaInfo = serde_json::from_str(
        r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"mjpeg",
             "disposition":{"attached_pic":1}}],"format":{}}"#,
    )
    .unwrap();
    assert!(picture_only.validate().is_err());
}

#[test]
fn test_probe_refuses_network_access_from_local_manifest() {
    use airmedia::media::check_dependencies;
    use airmedia::media::probe::MediaInfo;

    if !check_dependencies().is_ready() {
        return;
    }
    let dir = temp_dir("hls");
    let evil = dir.join("evil.m3u8");
    std::fs::write(
        &evil,
        "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1,\nhttp://127.0.0.1:9/segment.ts\n#EXT-X-ENDLIST\n",
    )
    .unwrap();

    let result = MediaInfo::probe(&evil).and_then(|info| info.validate().map(|_| info));
    assert!(result.is_err(), "a local manifest must not be able to pull network segments");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_filter_builders() {
    use airmedia::media::audio::atempo_chain;
    use airmedia::media::decoder::build_video_filter;
    use airmedia::media::{equalizer_filter, video_adjust_filter};

    assert_eq!(equalizer_filter(0.0, &[0.0; 10]), None);
    let mut gains = [0.0; 10];
    gains[0] = 6.0;
    assert_eq!(
        equalizer_filter(-2.0, &gains).as_deref(),
        Some("volume=-2.0dB,equalizer=f=31:t=o:w=1:g=6.0")
    );

    assert_eq!(video_adjust_filter(0.0, 1.0, 1.0, 1.0), None);
    assert!(video_adjust_filter(0.1, 1.0, 1.0, 1.0).unwrap().starts_with("eq=brightness=0.100"));

    assert_eq!(atempo_chain(1.0), None);
    assert_eq!(atempo_chain(0.25).as_deref(), Some("atempo=0.5,atempo=0.5000"));
    assert_eq!(atempo_chain(0.2).as_deref(), Some("atempo=0.5,atempo=0.5,atempo=0.8000"));
    assert_eq!(atempo_chain(2.5).as_deref(), Some("atempo=2.0,atempo=1.2500"));

    assert_eq!(build_video_filter(640, 360, None), "scale=640:360");
    assert_eq!(build_video_filter(640, 360, Some("eq=gamma=1.2")), "eq=gamma=1.2,scale=640:360");
}

#[test]
fn test_equalizer_state_filters() {
    use airmedia::ui::equalizer_dialog::{EqualizerPreset, EqualizerState};

    let mut eq = EqualizerState::default();
    assert_eq!(eq.audio_filter(), None);
    eq.band_gains_db = EqualizerPreset::BassBoost.gains();
    assert!(eq.audio_filter().unwrap().contains("equalizer=f=31"));
    eq.enabled = false;
    assert_eq!(eq.audio_filter(), None, "disabled equalizer adds no filter");
}

// ---------------------------------------------------------------------------
// Persistence and preferences helpers
// ---------------------------------------------------------------------------

#[test]
fn test_session_resume_and_recent_files() {
    use airmedia::persistence::{SessionState, MAX_RECENT_FILES};

    let mut s = SessionState::default();
    let movie = Path::new("movie.mkv");

    s.record_position(movie, 10.0, 600.0);
    assert_eq!(s.resume_position(movie), None, "too early to resume");
    s.record_position(movie, 120.0, 600.0);
    assert_eq!(s.resume_position(movie), Some(120.0));
    s.record_position(movie, 590.0, 600.0);
    assert_eq!(s.resume_position(movie), None, "finished items are forgotten");

    for i in 0..(MAX_RECENT_FILES + 5) {
        s.push_recent(Path::new(&format!("f{i}.mp4")));
    }
    s.push_recent(Path::new("f3.mp4"));
    assert_eq!(s.recent_files.len(), MAX_RECENT_FILES);
    assert_eq!(s.recent_files[0], PathBuf::from("f3.mp4"), "most recent first");
    assert_eq!(s.recent_files.iter().filter(|p| p.as_path() == Path::new("f3.mp4")).count(), 1);

    let back: SessionState = serde_json::from_str(&s.to_json()).unwrap();
    assert_eq!(back.recent_files, s.recent_files);
}

#[test]
fn test_preferences_tolerate_missing_fields_and_shorten_unicode_paths() {
    use airmedia::media::decoder::HwAccelMode;
    use airmedia::ui::preferences_dialog::{shorten_path_label, PreferencesState};

    // Older preference files without newer keys must still load.
    let prefs: PreferencesState = serde_json::from_str(r#"{"subtitle_size": 30.0}"#).unwrap();
    assert_eq!(prefs.subtitle_size, 30.0);
    assert_eq!(prefs.hw_accel, HwAccelMode::Auto);

    let label = shorten_path_label("C:\\Users\\Jos\u{e9}\\Im\u{e1}genes\\Capturas de pantalla", 22);
    assert_eq!(label.chars().count(), 22);
    assert!(label.starts_with('\u{2026}'));
    assert_eq!(shorten_path_label("short", 22), "short");
}

// ---------------------------------------------------------------------------
// Playlist reorder / multi-remove bookkeeping
// ---------------------------------------------------------------------------

fn titles(pl: &airmedia::playlist::Playlist) -> Vec<usize> {
    pl.items
        .iter()
        .map(|i| i.title.trim_start_matches("track").trim_end_matches(".mp3").parse().unwrap())
        .collect()
}

#[test]
fn test_playlist_move_single_item_down_and_up() {
    let mut pl = playlist_of(5);
    pl.current_index = Some(1);

    // Drag item 1 to just before item 4.
    let moved = pl.move_items(&[1], 4);
    assert_eq!(titles(&pl), vec![0, 2, 3, 1, 4]);
    assert_eq!(moved, 3..4);
    assert_eq!(pl.current_index, Some(3), "current item follows its move");

    // Drag item 4 to the top.
    pl.move_items(&[4], 0);
    assert_eq!(titles(&pl), vec![4, 0, 2, 3, 1]);
    assert_eq!(pl.current_index, Some(4));
    assert_eq!(pl.current_item().unwrap().title, "track1.mp3");

    // Dropping onto its own slot (or the one after it) is a no-op.
    let before = titles(&pl);
    pl.move_items(&[2], 2);
    pl.move_items(&[2], 3);
    assert_eq!(titles(&pl), before);
}

#[test]
fn test_playlist_move_updates_index_of_items_it_passes() {
    let mut pl = playlist_of(5);
    pl.current_index = Some(2);
    pl.move_items(&[0], 5); // first item to the end: everything shifts up
    assert_eq!(titles(&pl), vec![1, 2, 3, 4, 0]);
    assert_eq!(pl.current_index, Some(1));
    assert_eq!(pl.current_item().unwrap().title, "track2.mp3");

    pl.move_items(&[4], 0); // and back: everything shifts down
    assert_eq!(pl.current_index, Some(2));
    assert_eq!(pl.current_item().unwrap().title, "track2.mp3");
}

#[test]
fn test_playlist_move_block_keeps_order_selection_and_current() {
    let mut pl = playlist_of(7);
    pl.current_index = Some(5);
    pl.selection = [1, 3, 5].into();

    let moved = pl.move_items(&[1, 3, 5], 7);
    assert_eq!(titles(&pl), vec![0, 2, 4, 6, 1, 3, 5]);
    assert_eq!(moved, 4..7);
    assert_eq!(pl.current_item().unwrap().title, "track5.mp3");
    assert_eq!(pl.selection.iter().copied().collect::<Vec<_>>(), vec![4, 5, 6], "selection follows items");

    // Move the block into the middle; out-of-range indices are ignored.
    pl.move_items(&[4, 5, 6, 99], 2);
    assert_eq!(titles(&pl), vec![0, 2, 1, 3, 5, 4, 6]);
    assert_eq!(pl.current_item().unwrap().title, "track5.mp3");
    assert_eq!(pl.selection.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4]);
}

#[test]
fn test_playlist_remove_many_bookkeeping() {
    // Current item survives: index shifts by the number removed before it.
    let mut pl = playlist_of(6);
    pl.current_index = Some(4);
    pl.selection = [0, 2, 4].into();
    pl.remove_many(&[0, 2, 2, 42]);
    assert_eq!(titles(&pl), vec![1, 3, 4, 5]);
    assert_eq!(pl.current_item().unwrap().title, "track4.mp3");
    assert_eq!(pl.selection.iter().copied().collect::<Vec<_>>(), vec![2], "removed entries leave the selection");

    // Current item removed: moves to the next survivor.
    let mut pl = playlist_of(6);
    pl.current_index = Some(2);
    pl.remove_many(&[1, 2, 3]);
    assert_eq!(titles(&pl), vec![0, 4, 5]);
    assert_eq!(pl.current_item().unwrap().title, "track4.mp3");

    // Current item and everything after it removed: clamps to the last item.
    let mut pl = playlist_of(4);
    pl.current_index = Some(2);
    pl.remove_many(&[2, 3]);
    assert_eq!(pl.current_index, Some(1));

    // Everything removed.
    let mut pl = playlist_of(3);
    pl.current_index = Some(0);
    pl.remove_many(&[0, 1, 2]);
    assert!(pl.items.is_empty());
    assert_eq!(pl.current_index, None);

    // Single remove keeps its old semantics.
    let mut pl = playlist_of(3);
    pl.current_index = Some(2);
    pl.remove(2);
    assert_eq!(pl.current_index, Some(1));
    pl.remove(0);
    assert_eq!(pl.current_index, Some(0));
    assert_eq!(pl.current_item().unwrap().title, "track1.mp3");
}

#[test]
fn test_playlist_shuffle_history_survives_reorder() {
    let mut pl = playlist_of(5);
    pl.is_shuffle = true;
    pl.current_index = Some(0);
    let first_next = pl.next().unwrap().title.clone();
    // Reorder so every index changes; Previous must still return to track0.
    pl.move_items(&[4], 0);
    assert_eq!(pl.current_item().unwrap().title, first_next);
    assert_eq!(pl.prev().unwrap().title, "track0.mp3");
}

// ---------------------------------------------------------------------------
// A-B loop, jump-to-time parsing and the shortcut table
// ---------------------------------------------------------------------------

#[test]
fn test_ab_loop_points() {
    use airmedia::ab_loop::AbLoop;

    let mut ab = AbLoop::default();
    assert!(!ab.set_b(5.0), "B needs A first");
    ab.set_a(10.0);
    assert!(!ab.set_b(9.0), "B before A is refused");
    assert!(!ab.set_b(10.05), "a near-empty loop is refused");
    assert!(ab.set_b(20.0));
    assert!(ab.is_active());
    assert_eq!(ab.jump_target(15.0), None);
    assert_eq!(ab.jump_target(20.0), Some(10.0));
    assert_eq!(ab.jump_target(600.0), Some(10.0), "end of media inside the loop jumps back");

    ab.set_a(25.0);
    assert_eq!(ab.b, None, "moving A past B drops B");
    ab.clear();
    assert_eq!(ab, AbLoop::default());
}

#[test]
fn test_parse_clock() {
    use airmedia::ui::jump_dialog::parse_clock;

    assert_eq!(parse_clock("90"), Some(90.0));
    assert_eq!(parse_clock("1:30"), Some(90.0));
    assert_eq!(parse_clock(" 1:02:03 "), Some(3723.0));
    assert_eq!(parse_clock("0:05.5"), Some(5.5));
    assert_eq!(parse_clock("120:00"), Some(7200.0), "minutes may exceed 59 when they lead");
    for bad in ["", "1:", ":30", "1:60", "1:60:00", "a:b", "-5", "1:2:3:4", "NaN"] {
        assert_eq!(parse_clock(bad), None, "{bad:?} should be rejected");
    }
}

#[test]
fn test_readme_shortcut_table_matches_source() {
    let readme = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))
        .unwrap()
        .replace("\r\n", "\n");
    let table = airmedia::shortcuts::markdown_table();
    assert!(
        readme.contains(&table),
        "README shortcut table is out of date; replace it with:\n{table}"
    );
    let hints: Vec<_> = airmedia::shortcuts::hints().collect();
    assert!(hints.iter().any(|(k, _)| k.contains("F1")), "empty state advertises the help overlay");
}

// ---------------------------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------------------------

fn lib_paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn display_title_cleans_file_names() {
    use airmedia::library::display_title;
    assert_eq!(display_title(Path::new("/m/The.Long_Road.2019.mkv")), "The Long Road 2019");
    assert_eq!(display_title(Path::new("/m/Vol. 2 - Intro.mp3")), "Vol. 2 - Intro");
    assert_eq!(display_title(Path::new("https://example.com/live/my_show.m3u8?token=1")), "my show");
    assert_eq!(display_title(Path::new("https://example.com/")), "https://example.com/");
}

#[test]
fn media_kind_follows_extension_and_url() {
    use airmedia::library::MediaKind;
    assert_eq!(MediaKind::of(Path::new("a.FLAC")), MediaKind::Audio);
    assert_eq!(MediaKind::of(Path::new("a.mkv")), MediaKind::Video);
    assert_eq!(MediaKind::of(Path::new("https://x.test/a.mp3")), MediaKind::Stream);
}

#[test]
fn library_merges_sources_without_duplicates() {
    use airmedia::library::Library;
    let playlist = lib_paths(&["/m/a.mkv", "/m/b.mp3"]);
    let recent = lib_paths(&["/m/b.mp3", "/m/c.mp4"]);
    let resume = lib_paths(&["/m/c.mp4"]);
    let lib = Library::build(&playlist, &recent, &resume, &mut |_| true);

    assert_eq!(lib.entries.len(), 3);
    assert_eq!(lib.videos.len(), 2);
    assert_eq!(lib.music.len(), 1);
    let titles: Vec<&str> = lib.rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, ["Pick Up Where You Left Off", "Your Playlist", "Recently Played", "Videos", "Music"]);
    // c.mp4 is in the resume row, so "Recently Played" doesn't repeat it.
    let recent_row = &lib.rows[2];
    assert_eq!(recent_row.items.len(), 1);
    assert_eq!(lib.entries[recent_row.items[0]].path, PathBuf::from("/m/b.mp3"));
}

#[test]
fn library_hides_missing_history_but_keeps_streams() {
    use airmedia::library::Library;
    let recent = lib_paths(&["/gone/a.mkv", "https://x.test/live"]);
    let lib = Library::build(&[], &recent, &[], &mut |_| false);
    assert_eq!(lib.entries.len(), 1);
    assert_eq!(lib.entries[0].path, PathBuf::from("https://x.test/live"));
}

#[test]
fn library_folder_rows_need_two_folders_and_enough_items() {
    use airmedia::library::Library;
    let one_folder = lib_paths(&["/a/1.mp4", "/a/2.mp4", "/a/3.mp4"]);
    let lib = Library::build(&one_folder, &[], &[], &mut |_| true);
    assert!(lib.rows.iter().all(|r| !r.title.starts_with("From ")));

    let two_folders = lib_paths(&["/a/10.mp4", "/a/2.mp4", "/a/3.mp4", "/b/1.mp4"]);
    let lib = Library::build(&two_folders, &[], &[], &mut |_| true);
    let row = lib.rows.iter().find(|r| r.title == "From a").expect("row for folder a");
    let order: Vec<&str> = row.items.iter().map(|i| lib.entries[*i].title.as_str()).collect();
    assert_eq!(order, ["2", "3", "10"], "natural order");
    assert!(lib.rows.iter().all(|r| r.title != "From b"), "too small for a row");
}

#[test]
fn library_featured_prefers_playing_then_resume_then_next() {
    use airmedia::library::{FeaturedReason, Library};
    let playlist = lib_paths(&["/m/a.mkv", "/m/b.mkv"]);
    let resume = lib_paths(&["/m/c.mkv"]);
    let lib = Library::build(&playlist, &[], &resume, &mut |_| true);

    let a = Path::new("/m/a.mkv");
    let (idx, why) = lib.featured(Some(a), None).unwrap();
    assert_eq!((lib.entries[idx].path.as_path(), why), (a, FeaturedReason::NowPlaying));
    let (idx, why) = lib.featured(None, Some(Path::new("/m/b.mkv"))).unwrap();
    assert_eq!((lib.entries[idx].path.as_path(), why), (Path::new("/m/c.mkv"), FeaturedReason::Continue));

    let no_resume = Library::build(&playlist, &[], &[], &mut |_| true);
    let (idx, why) = no_resume.featured(None, Some(Path::new("/m/b.mkv"))).unwrap();
    assert_eq!((no_resume.entries[idx].path.as_path(), why), (Path::new("/m/b.mkv"), FeaturedReason::UpNext));
    assert!(Library::default().featured(None, None).is_none());
}

#[test]
fn library_search_matches_all_terms_in_title_and_folder() {
    use airmedia::library::Library;
    let playlist = lib_paths(&["/Holiday/beach_day.mp4", "/Work/beach-report.pdf.mp4", "/Holiday/city.mp4"]);
    let lib = Library::build(&playlist, &[], &[], &mut |_| true);
    assert_eq!(lib.search("BEACH").len(), 2);
    assert_eq!(lib.search("holiday beach").len(), 1);
    assert!(lib.search("   ").is_empty());
    assert!(lib.search("nothing").is_empty());
}

#[test]
fn format_duration_short_picks_a_compact_form() {
    use airmedia::library::format_duration_short;
    assert_eq!(format_duration_short(187.0), "3:07");
    assert_eq!(format_duration_short(42.0 * 60.0), "42m");
    assert_eq!(format_duration_short(3600.0), "1h");
    assert_eq!(format_duration_short(3600.0 + 42.0 * 60.0), "1h 42m");
}

#[test]
fn peek_next_matches_what_auto_advance_does() {
    use airmedia::playlist::{Playlist, RepeatMode};
    let mut pl = Playlist::new();
    pl.add("/m/a.mkv");
    pl.add("/m/b.mkv");
    assert!(pl.peek_next().is_none(), "nothing playing yet");

    pl.current_index = Some(0);
    assert_eq!(pl.peek_next().unwrap().title, "b.mkv");
    pl.current_index = Some(1);
    assert!(pl.peek_next().is_none(), "end of list");
    pl.repeat_mode = RepeatMode::All;
    assert_eq!(pl.peek_next().unwrap().title, "a.mkv");
    pl.repeat_mode = RepeatMode::One;
    assert!(pl.peek_next().is_none());
    pl.repeat_mode = RepeatMode::Off;
    pl.is_shuffle = true;
    assert!(pl.peek_next().is_none(), "shuffle order isn't known ahead of time");
}

#[test]
fn stopped_position_survives_and_old_session_entries_still_load() {
    use airmedia::persistence::SessionState;
    let movie = Path::new("/m/movie.mkv");
    let mut s = SessionState::default();
    s.record_position(movie, 120.0, 600.0);
    assert_eq!(s.resume_progress(movie), Some((120.0, 600.0)));
    assert_eq!(s.resume_paths(), vec![movie.to_path_buf()]);
    s.forget_position(movie);
    assert_eq!(s.resume_progress(movie), None);

    // Sessions written before durations were stored.
    let old = r#"{"resume":[{"path":"/m/movie.mkv","position_secs":90.0}]}"#;
    let loaded: SessionState = serde_json::from_str(old).unwrap();
    assert_eq!(loaded.resume_progress(movie), Some((90.0, 0.0)));
}
