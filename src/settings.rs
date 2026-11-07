//! The few things the app remembers between runs, in `%APPDATA%\TronicPrinter\settings.txt` as
//! `key=value` lines. No serde, no surprises: a missing or garbled file just means defaults.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Settings {
    /// `None`: never chosen, so follow Windows' animation setting.
    pub effects: Option<bool>,
    /// The folder shown in the picture gallery.
    pub gallery: Option<PathBuf>,
    /// Print new pictures that arrive in the gallery folder.
    pub auto_print: bool,
    /// The port of the app's HTTP MCP endpoint; `None` means the default.
    pub port: Option<u16>,
    /// Ids of the starred writing themes, in the order they were starred.
    pub fav_themes: Vec<String>,
    /// The starred pictures (full paths), in the order they were starred.
    pub fav_pictures: Vec<PathBuf>,
    /// The id of the colour palette (see `style::PaletteId`); empty when never chosen.
    pub palette: String,
}

pub fn path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("TronicPrinter").join("settings.txt"))
}

pub fn parse(text: &str) -> Settings {
    let mut s = Settings::default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("effects=") {
            s.effects = match v.trim() {
                "on" => Some(true),
                "off" => Some(false),
                _ => None,
            };
        } else if let Some(v) = line.strip_prefix("gallery=") {
            // The path may contain '=' itself, so take everything after the first one.
            if !v.trim().is_empty() {
                s.gallery = Some(PathBuf::from(v.trim()));
            }
        } else if let Some(v) = line.strip_prefix("autoprint=") {
            s.auto_print = v.trim() == "on";
        } else if let Some(v) = line.strip_prefix("port=") {
            s.port = v.trim().parse().ok().filter(|p| *p != 0);
        } else if let Some(v) = line.strip_prefix("palette=") {
            s.palette = v.trim().to_ascii_lowercase();
        } else if let Some(v) = line.strip_prefix("fav-theme=") {
            let id = v.trim();
            if !id.is_empty() && !s.fav_themes.iter().any(|t| t == id) {
                s.fav_themes.push(id.to_string());
            }
        } else if let Some(v) = line.strip_prefix("fav-picture=") {
            let path = PathBuf::from(v.trim());
            if !v.trim().is_empty() && !s.fav_pictures.contains(&path) {
                s.fav_pictures.push(path);
            }
        }
    }
    s
}

pub fn render(s: &Settings) -> String {
    let mut out = String::new();
    match s.effects {
        Some(true) => out.push_str("effects=on\n"),
        Some(false) => out.push_str("effects=off\n"),
        None => {}
    }
    if let Some(dir) = &s.gallery {
        out.push_str(&format!("gallery={}\n", dir.display()));
    }
    if s.auto_print {
        out.push_str("autoprint=on\n");
    }
    if let Some(port) = s.port {
        out.push_str(&format!("port={port}\n"));
    }
    if !s.palette.is_empty() {
        out.push_str(&format!("palette={}\n", s.palette));
    }
    for id in &s.fav_themes {
        out.push_str(&format!("fav-theme={id}\n"));
    }
    for p in &s.fav_pictures {
        out.push_str(&format!("fav-picture={}\n", p.display()));
    }
    out
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

/// Change one setting and keep the others.
pub fn update(f: impl FnOnce(&mut Settings)) {
    let mut s = load();
    f(&mut s);
    save(&s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_garbage_means_defaults() {
        for effects in [Some(true), Some(false), None] {
            for auto_print in [true, false] {
                for port in [Some(8080), None] {
                    let s = Settings {
                        effects,
                        gallery: Some(PathBuf::from(r"C:\Users\me\Pictures\memes = fun")),
                        auto_print,
                        port,
                        fav_themes: vec!["receipt".into(), "diner".into()],
                        fav_pictures: vec![PathBuf::from(r"C:\Users\me\Pictures\a = b.png")],
                        palette: "night".into(),
                    };
                    assert_eq!(parse(&render(&s)), s);
                }
            }
        }
        assert_eq!(parse("effects=maybe\nnonsense\ngallery=\nautoprint=yes\nport=0"), Settings::default());
        assert_eq!(parse("port=70000"), Settings::default(), "out of range is ignored");
        assert_eq!(parse("fav-theme=\nfav-picture=").fav_themes.len(), 0, "empty favorites are ignored");
        assert_eq!(parse("fav-theme=a\nfav-theme=a").fav_themes, ["a"], "no duplicates");
        assert_eq!(parse(""), Settings::default());
    }

    #[test]
    fn saving_creates_the_folder_and_loading_reads_it_back() {
        let dir = std::env::temp_dir().join(format!("tronic-settings-test-{}", std::process::id()));
        let file = dir.join("nested").join("settings.txt");
        let s = Settings { effects: Some(false), gallery: Some(PathBuf::from("memes")), auto_print: true, port: Some(9000), fav_themes: vec!["fortune".into()], fav_pictures: vec![], palette: "matrix".into() };
        save_to(&file, &s);
        assert_eq!(load_from(&file), s);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load_from(&file), Settings::default(), "a missing file is just defaults");
    }

    #[test]
    fn the_palette_is_kept_as_its_id() {
        assert_eq!(parse("").palette, "", "never chosen");
        assert_eq!(parse("palette= Halloween \n").palette, "halloween");
        let s = Settings { palette: "matrix".into(), ..Settings::default() };
        assert_eq!(parse(&render(&s)).palette, "matrix");
        assert!(!render(&Settings::default()).contains("palette"), "nothing chosen, nothing written");
    }
}
