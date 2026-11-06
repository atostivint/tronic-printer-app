//! The few things the app remembers between runs, in `%APPDATA%\TronicPrinter\settings.txt` as
//! `key=value` lines. No serde, no surprises: a missing or garbled file just means defaults.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Settings {
    /// `None`: never chosen, so follow Windows' animation setting.
    pub effects: Option<bool>,
}

pub fn path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("TronicPrinter").join("settings.txt"))
}

pub fn parse(text: &str) -> Settings {
    let mut s = Settings::default();
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("effects=") {
            s.effects = match v.trim() {
                "on" => Some(true),
                "off" => Some(false),
                _ => None,
            };
        }
    }
    s
}

pub fn render(s: &Settings) -> String {
    match s.effects {
        Some(true) => "effects=on\n".into(),
        Some(false) => "effects=off\n".into(),
        None => String::new(),
    }
}

pub fn load_from(path: &Path) -> Settings {
    std::fs::read_to_string(path).map(|t| parse(&t)).unwrap_or_default()
}

pub fn save_to(path: &Path, s: &Settings) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, render(s));
}

pub fn load() -> Settings {
    path().map(|p| load_from(&p)).unwrap_or_default()
}

pub fn save(s: &Settings) {
    if let Some(p) = path() {
        save_to(&p, s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_garbage_means_defaults() {
        for effects in [Some(true), Some(false), None] {
            let s = Settings { effects };
            assert_eq!(parse(&render(&s)), s);
        }
        assert_eq!(parse("effects=maybe\nnonsense"), Settings::default());
        assert_eq!(parse(""), Settings::default());
    }

    #[test]
    fn saving_creates_the_folder_and_loading_reads_it_back() {
        let dir = std::env::temp_dir().join(format!("tronic-settings-test-{}", std::process::id()));
        let file = dir.join("nested").join("settings.txt");
        save_to(&file, &Settings { effects: Some(false) });
        assert_eq!(load_from(&file), Settings { effects: Some(false) });
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load_from(&file), Settings::default(), "a missing file is just defaults");
    }
}
