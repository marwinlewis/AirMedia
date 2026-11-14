//! Incognito windows must never write history to disk. Kept in its own test binary because it
//! points the per-user config directory at a scratch folder through the environment.

use airmedia::app::AirMediaApp;
use airmedia::persistence::{config_dir, SessionState};
use std::path::PathBuf;

fn sandbox_config_dir() -> PathBuf {
    let root = std::env::temp_dir().join(format!("airmedia-incognito-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    // This binary has a single test, so nothing else reads the environment concurrently.
    std::env::set_var("APPDATA", &root);
    std::env::set_var("XDG_CONFIG_HOME", &root);
    root
}

#[test]
fn incognito_starts_blank_and_saves_nothing() {
    let root = sandbox_config_dir();
    let session_file = config_dir().unwrap().join("session.json");

    // A normal run with some history.
    let mut saved = SessionState::default();
    saved.push_recent(&PathBuf::from("https://example.com/watched-before.m3u8"));
    saved.playlist = vec![PathBuf::from("https://example.com/queued.m3u8")];
    saved.equalizer.enabled = true;
    std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
    std::fs::write(&session_file, saved.to_json()).unwrap();
    let before = std::fs::read_to_string(&session_file).unwrap();

    {
        let mut app = AirMediaApp::new(true);
        assert!(app.incognito);
        assert!(app.session.recent_files.is_empty(), "incognito must not show earlier history");
        assert!(app.playlist.items.is_empty(), "incognito must not restore the playlist");
        assert!(app.session.equalizer.enabled, "effect settings still apply");

        // Watch something, then save (periodic save and the save on drop take this path).
        let watched = PathBuf::from("https://example.com/private.m3u8");
        app.session.push_recent(&watched);
        app.playlist.add(&watched);
        app.save_session();
    }

    assert_eq!(std::fs::read_to_string(&session_file).unwrap(), before, "session.json must be untouched");

    // A normal window still saves.
    {
        let mut app = AirMediaApp::new(false);
        assert_eq!(app.session.recent_files.len(), 1);
        app.session.push_recent(&PathBuf::from("https://example.com/normal.m3u8"));
        app.save_session();
    }
    assert_ne!(std::fs::read_to_string(&session_file).unwrap(), before);

    let _ = std::fs::remove_dir_all(root);
}
