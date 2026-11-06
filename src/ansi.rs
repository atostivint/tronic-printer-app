//! Cleans terminal output for paper: no colour codes, no cursor movement, no progress-bar
//! rewrites.

/// Strip ANSI escape sequences and control characters. A lone carriage return rewrites the
/// current line, the way a terminal shows a progress bar, so only its last version is kept.
pub fn clean(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut line_start = 0;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.peek() {
                Some('[') => {
                    chars.next();
                    for n in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(n) = chars.next() {
                        if n == '\x07' {
                            break;
                        }
                        if n == '\x1b' {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            },
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    continue;
                }
                out.truncate(line_start);
            }
            '\n' => {
                out.push('\n');
                line_start = out.len();
            }
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn colours_and_cursor_codes_vanish() {
        assert_eq!(clean("\x1b[31mred\x1b[0m plain\x1b[2K"), "red plain");
    }

    #[test]
    fn a_progress_bar_keeps_only_its_last_state() {
        assert_eq!(clean("10%\r50%\r100%\ndone\n"), "100%\ndone\n");
    }

    #[test]
    fn windows_line_endings_are_kept_as_lines() {
        assert_eq!(clean("a\r\nb\r\n"), "a\nb\n");
    }

    #[test]
    fn titles_and_tabs_and_bells_are_handled() {
        assert_eq!(clean("\x1b]0;title\x07x\ty\x07"), "x    y");
        assert_eq!(clean("é ü 日本"), "é ü 日本");
    }
}
