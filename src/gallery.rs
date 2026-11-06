//! The picture gallery: a folder you choose, its pictures as thumbnails, and an optional watcher
//! that prints new pictures dropped into the folder by itself.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use rune::Image;

pub const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "bmp", "gif", "webp"];
/// Pictures read from a folder, newest first. A folder with more is cut here.
const MAX_ENTRIES: usize = 500;
/// Thumbnails are made for this many entries, newest first.
pub const MAX_THUMBS: usize = 48;
/// How often the folder is looked at.
pub const SCAN_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub modified: SystemTime,
}

pub fn is_image(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// The pictures in `dir`, newest first. An unreadable folder is just empty.
pub fn scan(dir: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !is_image(&path) {
                return None;
            }
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            Some(Entry { name: path.file_name()?.to_string_lossy().into_owned(), path, size: meta.len(), modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH) })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    out.truncate(MAX_ENTRIES);
    out
}

/// Spots pictures that arrive in a folder. The first look only takes note of what is there, so
/// pictures already in the folder never print; a new one counts once its size has stopped
/// changing between two looks (it is not half-copied any more).
#[derive(Default)]
pub struct Watch {
    seen: HashSet<PathBuf>,
    growing: HashMap<PathBuf, u64>,
    primed: bool,
}

impl Watch {
    pub fn reset(&mut self) {
        *self = Watch::default();
    }

    pub fn arrivals(&mut self, entries: &[Entry]) -> Vec<PathBuf> {
        if !self.primed {
            self.seen.extend(entries.iter().map(|e| e.path.clone()));
            self.primed = true;
            return Vec::new();
        }
        let mut arrived = Vec::new();
        for e in entries {
            if self.seen.contains(&e.path) {
                continue;
            }
            if self.growing.get(&e.path) == Some(&e.size) && e.size > 0 {
                self.growing.remove(&e.path);
                self.seen.insert(e.path.clone());
                arrived.push(e.path.clone());
            } else {
                self.growing.insert(e.path.clone(), e.size);
            }
        }
        // A file that vanished before it settled is forgotten.
        self.growing.retain(|p, _| entries.iter().any(|e| &e.path == p));
        arrived
    }
}

/// Everything the gallery keeps.
#[derive(Default)]
pub struct Gallery {
    pub dir: Option<PathBuf>,
    pub entries: Vec<Entry>,
    pub thumbs: HashMap<PathBuf, Image>,
    /// Print new pictures as they arrive.
    pub auto: bool,
    pub watch: Watch,
    pub last_scan: Option<Instant>,
    /// Pictures waiting for the printer to be free.
    pub queue: VecDeque<PathBuf>,
    /// How many pictures were printed by the watcher since it was switched on.
    pub auto_printed: usize,
    pub selected: Option<PathBuf>,
}

impl Gallery {
    pub fn set_dir(&mut self, dir: PathBuf) {
        self.dir = Some(dir);
        self.entries.clear();
        self.thumbs.clear();
        self.queue.clear();
        self.watch.reset();
        self.last_scan = None;
        self.selected = None;
    }

    /// Look at the folder if it is time. Returns true when the list of pictures changed, plus
    /// the pictures that just arrived.
    pub fn rescan(&mut self, now: Instant) -> (bool, Vec<PathBuf>) {
        let Some(dir) = &self.dir else { return (false, Vec::new()) };
        if self.last_scan.is_some_and(|t| now.duration_since(t) < SCAN_EVERY) {
            return (false, Vec::new());
        }
        self.last_scan = Some(now);
        let entries = scan(dir);
        let arrived = self.watch.arrivals(&entries);
        let changed = entries != self.entries;
        if changed {
            self.entries = entries;
            let keep: HashSet<&PathBuf> = self.entries.iter().take(MAX_THUMBS).map(|e| &e.path).collect();
            self.thumbs.retain(|p, _| keep.contains(p));
        }
        (changed, arrived)
    }

    pub fn folder_name(&self) -> Option<String> {
        let dir = self.dir.as_ref()?;
        Some(dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dir.display().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tronic-gallery-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn only_pictures_are_listed_newest_first() {
        let dir = temp_dir("scan");
        fs::write(dir.join("a.PNG"), b"x").unwrap();
        fs::write(dir.join("notes.txt"), b"x").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        fs::write(dir.join("b.jpg"), b"xx").unwrap();
        fs::create_dir_all(dir.join("sub.png")).unwrap();
        let names: Vec<String> = scan(&dir).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["b.jpg", "a.PNG"], "a folder named like a picture is not one");
        assert!(scan(&dir.join("missing")).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    fn entry(name: &str, size: u64) -> Entry {
        Entry { path: PathBuf::from(name), name: name.into(), size, modified: SystemTime::UNIX_EPOCH }
    }

    #[test]
    fn pictures_already_there_never_print_and_new_ones_wait_until_they_stop_growing() {
        let mut w = Watch::default();
        assert!(w.arrivals(&[entry("old.png", 10)]).is_empty(), "the first look only takes note");
        assert!(w.arrivals(&[entry("old.png", 10)]).is_empty());
        assert!(w.arrivals(&[entry("new.png", 100), entry("old.png", 10)]).is_empty(), "just appeared");
        assert!(w.arrivals(&[entry("new.png", 400), entry("old.png", 10)]).is_empty(), "still being copied");
        assert_eq!(w.arrivals(&[entry("new.png", 400), entry("old.png", 10)]), [PathBuf::from("new.png")], "settled");
        assert!(w.arrivals(&[entry("new.png", 400), entry("old.png", 10)]).is_empty(), "and only once");
    }

    #[test]
    fn an_empty_file_and_a_vanished_one_never_count() {
        let mut w = Watch::default();
        w.arrivals(&[]);
        assert!(w.arrivals(&[entry("zero.png", 0)]).is_empty());
        assert!(w.arrivals(&[entry("zero.png", 0)]).is_empty(), "empty is not done");
        assert!(w.arrivals(&[]).is_empty());
        assert!(w.arrivals(&[entry("zero.png", 5)]).is_empty(), "it starts over after it vanished");
    }

    #[test]
    fn the_gallery_looks_at_its_folder_every_two_seconds_and_reports_arrivals() {
        let dir = temp_dir("rescan");
        let mut g = Gallery::default();
        let t0 = Instant::now();
        assert_eq!(g.rescan(t0), (false, vec![]), "no folder, nothing to do");
        fs::write(dir.join("one.png"), b"1").unwrap();
        g.set_dir(dir.clone());
        let (changed, arrived) = g.rescan(t0);
        assert!(changed && arrived.is_empty());
        assert_eq!(g.entries.len(), 1);
        assert_eq!(g.rescan(t0 + Duration::from_millis(500)), (false, vec![]), "too soon");
        fs::write(dir.join("two.png"), b"22").unwrap();
        let (changed, arrived) = g.rescan(t0 + SCAN_EVERY);
        assert!(changed && arrived.is_empty(), "first sight of a new file");
        let (_, arrived) = g.rescan(t0 + SCAN_EVERY * 2);
        assert_eq!(arrived, [dir.join("two.png")]);
        assert_eq!(g.folder_name().as_deref(), dir.file_name().and_then(|n| n.to_str()));
        let _ = fs::remove_dir_all(&dir);
    }
}
