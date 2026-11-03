//! The single table of keyboard shortcuts. It feeds the help overlay (`?` / F1), the
//! empty-state hints, and the README table (kept in sync by a test).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Playback,
    Seeking,
    AudioVideo,
    Playlist,
    Window,
}

impl Group {
    pub const ALL: [Group; 5] = [Group::Playback, Group::Seeking, Group::AudioVideo, Group::Playlist, Group::Window];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Playback => "Playback",
            Self::Seeking => "Seeking",
            Self::AudioVideo => "Audio & video",
            Self::Playlist => "Playlist",
            Self::Window => "Window & dialogs",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Shortcut {
    /// Keys as shown to the user, e.g. `Ctrl + G`.
    pub keys: &'static str,
    pub action: &'static str,
    pub group: Group,
    /// Short label for the empty-state hint row; only a few entries have one.
    pub hint: Option<&'static str>,
}

const fn sc(keys: &'static str, action: &'static str, group: Group) -> Shortcut {
    Shortcut { keys, action, group, hint: None }
}

const fn hint(keys: &'static str, action: &'static str, group: Group, label: &'static str) -> Shortcut {
    Shortcut { keys, action, group, hint: Some(label) }
}

pub const SHORTCUTS: &[Shortcut] = &[
    sc("Space", "Play / Pause", Group::Playback),
    sc("S", "Stop playback", Group::Playback),
    sc("N / P", "Next / Previous track in playlist", Group::Playback),
    sc("Media keys", "Play/Pause, Stop, Next and Previous track", Group::Playback),
    sc(".", "Step 1 frame forward", Group::Playback),
    sc(",", "Step 1 frame backward", Group::Playback),
    sc("[ / ]", "Decrease / Increase playback speed (0.25x steps)", Group::Playback),
    sc("Backspace or =", "Reset playback speed to 1.0x", Group::Playback),
    sc("Left / Right", "Seek backward / forward 5s", Group::Seeking),
    sc("Shift + Left / Right", "Seek backward / forward 15s", Group::Seeking),
    sc("Ctrl + Left / Right", "Seek backward / forward 60s", Group::Seeking),
    sc("Ctrl + G", "Jump to time", Group::Seeking),
    sc("B", "A-B loop: set A, then B, then clear", Group::Seeking),
    sc("Up / Down", "Volume up / down (5%)", Group::AudioVideo),
    sc("M", "Mute / Unmute", Group::AudioVideo),
    sc("Mouse wheel", "Adjust volume over the video", Group::AudioVideo),
    sc("Ctrl + E", "Equalizer & video filters", Group::AudioVideo),
    sc("A", "Cycle aspect ratio (Fit, Fill, 16:9, 4:3, 21:9, Original)", Group::AudioVideo),
    sc("H", "Cycle hardware acceleration (Auto, D3D11VA, D3D12VA, CUDA, Vulkan, CPU)", Group::AudioVideo),
    sc("Shift + S", "Take screenshot", Group::AudioVideo),
    hint("L", "Toggle playlist drawer", Group::Playlist, "playlist"),
    sc("Delete", "Remove selected playlist items", Group::Playlist),
    sc("Ctrl / Shift + Click", "Select several playlist items", Group::Playlist),
    sc("Drag", "Reorder playlist items", Group::Playlist),
    hint("O", "Open media file", Group::Window, "open"),
    sc("Ctrl + N", "Open network stream", Group::Window),
    hint("F or F11", "Toggle fullscreen", Group::Window, "fullscreen"),
    sc("Double click", "Toggle fullscreen", Group::Window),
    sc("Esc", "Exit fullscreen / close dialog", Group::Window),
    sc("Ctrl + T", "Toggle always on top", Group::Window),
    sc("I", "Media information", Group::Window),
    sc("Ctrl + P", "Preferences", Group::Window),
    hint("? or F1", "Show keyboard shortcuts", Group::Window, "shortcuts"),
    sc("Ctrl + Q", "Quit", Group::Window),
];

pub fn in_group(group: Group) -> impl Iterator<Item = &'static Shortcut> {
    SHORTCUTS.iter().filter(move |s| s.group == group)
}

/// `(keys, label)` pairs for the empty-state hint row, in table order.
pub fn hints() -> impl Iterator<Item = (&'static str, &'static str)> {
    SHORTCUTS.iter().filter_map(|s| s.hint.map(|h| (s.keys, h)))
}

/// The README's shortcut table (Markdown, `\n` line endings).
pub fn markdown_table() -> String {
    let mut out = String::from("| Shortcut | Action |\n|---|---|\n");
    for group in Group::ALL {
        for s in in_group(group) {
            let keys = s
                .keys
                .split(" or ")
                .map(|alt| format!("`{alt}`"))
                .collect::<Vec<_>>()
                .join(" or ");
            out.push_str(&format!("| {keys} | {} |\n", s.action));
        }
    }
    out
}
