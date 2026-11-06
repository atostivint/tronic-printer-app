//! Easter eggs: the Konami code.

use rune::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pad {
    Up,
    Down,
    Left,
    Right,
    B,
    A,
}

const CODE: [Pad; 10] = [Pad::Up, Pad::Up, Pad::Down, Pad::Down, Pad::Left, Pad::Right, Pad::Left, Pad::Right, Pad::B, Pad::A];

/// The last ten pads pressed: the code is ↑ ↑ ↓ ↓ ← → ← → B A, however many wrong keys came
/// before it (so an extra ↑ at the start does no harm).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Konami {
    last: [Option<Pad>; 10],
}

impl Konami {
    /// Feed one key; true on the key that completes the code.
    pub fn feed(&mut self, pad: Pad) -> bool {
        self.last.rotate_left(1);
        self.last[CODE.len() - 1] = Some(pad);
        if self.last.iter().copied().eq(CODE.iter().copied().map(Some)) {
            *self = Konami::default();
            return true;
        }
        false
    }
}

/// The pad a key stands for, if any.
pub fn pad_of(key: &Key) -> Option<Pad> {
    match key {
        Key::Up => Some(Pad::Up),
        Key::Down => Some(Pad::Down),
        Key::Left => Some(Pad::Left),
        Key::Right => Some(Pad::Right),
        Key::Text(t) if t.eq_ignore_ascii_case("b") => Some(Pad::B),
        Key::Text(t) if t.eq_ignore_ascii_case("a") => Some(Pad::A),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &[Pad]) -> usize {
        let mut k = Konami::default();
        code.iter().filter(|p| k.feed(**p)).count()
    }

    #[test]
    fn the_code_fires_once_when_completed() {
        assert_eq!(run(&CODE), 1);
        let twice: Vec<Pad> = CODE.iter().chain(CODE.iter()).copied().collect();
        assert_eq!(run(&twice), 2);
    }

    #[test]
    fn a_wrong_key_restarts_but_an_extra_up_still_counts() {
        let mut wrong = CODE.to_vec();
        wrong.insert(3, Pad::A);
        assert_eq!(run(&wrong), 0);
        let mut extra_up = vec![Pad::Up];
        extra_up.extend(CODE);
        assert_eq!(run(&extra_up), 1, "↑ then the whole code still works");
    }

    #[test]
    fn keys_map_to_pads() {
        assert_eq!(pad_of(&Key::Up), Some(Pad::Up));
        assert_eq!(pad_of(&Key::Text("B".into())), Some(Pad::B));
        assert_eq!(pad_of(&Key::Text("a".into())), Some(Pad::A));
        assert_eq!(pad_of(&Key::Enter), None);
        assert_eq!(pad_of(&Key::Text("x".into())), None);
    }
}
