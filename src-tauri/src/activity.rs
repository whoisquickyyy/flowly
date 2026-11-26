use active_win_pos_rs::get_active_window;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInfo {
    pub category: String, // "flowly" | "coding" | "gaming" | "video" | "other"
    pub app_name: String,
    pub title: String,
}

pub fn detect_active_window(custom_games: &[String]) -> ActivityInfo {
    let Ok(win) = get_active_window() else {
        return ActivityInfo {
            category: "other".to_string(),
            app_name: String::new(),
            title: String::new(),
        };
    };

    let app_low = win.app_name.to_lowercase();
    let title_low = win.title.to_lowercase();
    let path_low = win.process_path.to_string_lossy().to_lowercase();

    let category = if app_low.contains("flowly") || app_low.contains("aura-music") || title_low.starts_with("flowly") {
        "flowly"
    } else if is_video_window(&app_low, &path_low, &title_low) {
        "video"
    } else if is_coding_window(&app_low, &path_low) {
        "coding"
    } else if is_gaming_window(&app_low, &path_low, custom_games) {
        "gaming"
    } else {
        "other"
    };

    ActivityInfo {
        category: category.to_string(),
        app_name: win.app_name,
        title: win.title,
    }
}

fn is_video_window(app_low: &str, path_low: &str, title_low: &str) -> bool {
    let is_browser = ["chrome", "msedge", "firefox", "opera", "yandex", "brave", "vivaldi", "arc"]
        .iter()
        .any(|b| app_low.contains(b) || path_low.contains(b));
    if !is_browser {
        return false;
    }
    ["youtube", "twitch", "кинопоиск", "vk видео", "netflix", "rutube", "tiktok"]
        .iter()
        .any(|k| title_low.contains(k))
}

fn is_coding_window(app_low: &str, path_low: &str) -> bool {
    let ides = [
        "visual studio code",
        "code.exe",
        "cursor",
        "idea64",
        "intellij",
        "pycharm",
        "clion",
        "rider",
        "webstorm",
        "goland",
        "rustrover",
        "devenv",
        "sublime_text",
        "zed",
    ];
    ides.iter().any(|id| app_low.contains(id) || path_low.contains(id))
}

fn is_gaming_window(app_low: &str, path_low: &str, custom_games: &[String]) -> bool {
    if custom_games
        .iter()
        .map(|g| g.trim().to_lowercase())
        .filter(|g| !g.is_empty())
        .any(|g| app_low.contains(&g) || path_low.contains(&g))
    {
        return true;
    }
    if path_low.contains("steamapps\\common")
        || path_low.contains("epic games\\")
        || path_low.contains("riot games\\")
    {
        return true;
    }
    let known = [
        "cs2",
        "dota2",
        "valorant",
        "gta5",
        "cyberpunk",
        "eldenring",
        "javaw",
        "minecraft",
        "roblox",
        "fortnite",
        "r5apex",
        "overwatch",
        "warthunder",
        "worldoftanks",
    ];
    known.iter().any(|g| app_low.contains(g) || path_low.contains(g))
}
