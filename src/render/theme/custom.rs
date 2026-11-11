//! Themes that live in files, so they can be added and removed without recompiling.
//!
//! A theme is a small JSON file in `%APPDATA%\TronicPrinter\themes\`. It names the theme, lists
//! the little form it asks for, and describes the ticket either as a stack of lines (`"layout":
//! "page"`, built with the same toolkit as the built-in themes) or as a sideways sheet with
//! absolute coordinates (`"layout": "sheet"`, turned to print down the paper). See
//! `themes/README.md` for the whole format.
//!
//! Loading never panics and never half-loads: a file with a mistake is skipped and reported, the
//! others still load. Loaded themes are leaked once (and reused while the file is unchanged), so a
//! [`Theme::Custom`] handle can hand out `&'static` names like the built-in ones do.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use rhai::{Dynamic, Engine, Scope};
use serde_json::Value;

use super::{Ctx, Family, Field, MAX_FIELDS, Page, Region, Theme, money, split_price, today};
use crate::protocol::Bitmap;
use crate::render::draw::{self, Anchor, Sheet};
use crate::render::text::{Align, FontChoice};
use crate::render::{self, CodeKind, CodeOpts};

/// One theme read from a file.
pub struct Custom {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    pub family: Family,
    pub fields: &'static [Field],
    pub sample: &'static str,
    pub body_optional: bool,
    layout: Layout,
}

enum Layout {
    Page { margin: f32, steps: Vec<Placed>, finish: Vec<Finish> },
    Sheet { w: usize, h: usize, ops: Vec<Op> },
}

#[derive(Clone, Copy)]
struct Style {
    font: FontChoice,
    size: f32,
    align: Align,
}

enum Step {
    Text { text: Tpl, st: Style, line_gap: f32, any: bool },
    Band { text: Tpl, st: Style },
    Shout { text: Tpl, font: FontChoice, max: f32 },
    Leader { left: Tpl, right: Tpl, font: FontChoice, size: f32 },
    Rule { dash: (usize, usize), thick: usize, space: usize },
    Flourish,
    Gap(usize),
    Barcode { data: Tpl, rows: usize },
    Code { data: Tpl, kind: CodeKind },
    Each(Vec<Placed>),
}

/// A step, and the field it is the writable part of when its author said so.
struct Placed {
    step: Step,
    region: Option<String>,
}

enum Finish {
    Border(usize),
    Speckle { count: usize, big: usize },
    Ragged(usize),
    Zigzag(usize),
    Tractor(usize),
}

enum Op {
    Rect { x0: i32, y0: i32, x1: i32, y1: i32, t: i32 },
    Fill { x0: i32, y0: i32, x1: i32, y1: i32 },
    HLine { y: i32, x0: i32, x1: i32, t: i32, dash: (usize, usize) },
    VLine { x: i32, y0: i32, y1: i32, t: i32, dash: (usize, usize) },
    Line { x0: i32, y0: i32, x1: i32, y1: i32, t: i32 },
    Text { text: Tpl, font: FontChoice, size: f32, x: i32, y: i32, anchor: Anchor, knock: bool, region: Option<String> },
    Bars { data: Tpl, x: i32, y: i32, h: i32, module: usize },
    Punch { x: i32, y: i32, r: i32 },
    Disc { x: i32, y: i32, r: i32, ring: Option<i32> },
    Round(usize),
}

// ---- templates: "{store}", "{line|upper}", "{total}"… --------------------------------------------

enum Var {
    Body,
    Line,
    Name,
    Price,
    Total,
    Today,
    Index,
    Number { digits: usize, salt: u64 },
    Field(String),
}

enum Piece {
    Lit(String),
    Var(Var, bool),
}

/// Text with `{…}` holes, checked when the file is read.
struct Tpl(Vec<Piece>);

struct Vars<'a> {
    ctx: &'a Ctx<'a>,
    fields: &'static [Field],
    line: Option<&'a str>,
    index: usize,
}

impl Tpl {
    fn parse(src: &str, keys: &[&str], in_each: bool) -> Result<Tpl, String> {
        let mut out = Vec::new();
        let mut lit = String::new();
        let mut chars = src.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    lit.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    lit.push('}');
                }
                '{' => {
                    let mut inner = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => inner.push(c),
                            None => return Err(format!("a '{{' is never closed in \"{src}\" (write '{{{{' for a literal one)")),
                        }
                    }
                    if !lit.is_empty() {
                        out.push(Piece::Lit(std::mem::take(&mut lit)));
                    }
                    out.push(parse_var(&inner, keys, in_each)?);
                }
                c => lit.push(c),
            }
        }
        if !lit.is_empty() {
            out.push(Piece::Lit(lit));
        }
        Ok(Tpl(out))
    }

    fn eval(&self, v: &Vars) -> String {
        let mut out = String::new();
        for piece in &self.0 {
            match piece {
                Piece::Lit(s) => out.push_str(s),
                Piece::Var(var, upper) => {
                    let s = value(var, v);
                    out.push_str(&if *upper { s.to_uppercase() } else { s });
                }
            }
        }
        out
    }
}

fn parse_var(inner: &str, keys: &[&str], in_each: bool) -> Result<Piece, String> {
    let (name, filter) = match inner.split_once('|') {
        Some((n, f)) => (n.trim(), Some(f.trim())),
        None => (inner.trim(), None),
    };
    let upper = match filter {
        None | Some("lower") => false,
        Some("upper") => true,
        Some(other) => return Err(format!("unknown filter '{other}' in {{{inner}}} (use upper)")),
    };
    let only_in_each = |v: Var| if in_each { Ok(v) } else { Err(format!("{{{name}}} only works inside an \"each\" step")) };
    let var = match name {
        "body" => Var::Body,
        "today" => Var::Today,
        "total" => Var::Total,
        "line" => only_in_each(Var::Line)?,
        "name" => only_in_each(Var::Name)?,
        "price" => only_in_each(Var::Price)?,
        "index" => only_in_each(Var::Index)?,
        n if n == "number" || n.starts_with("number:") => {
            let mut parts = n.split(':').skip(1);
            let digits = parts.next().map(|d| d.parse::<usize>().map_err(|_| format!("{{{n}}}: the digit count must be a number"))).transpose()?.unwrap_or(6);
            let salt = parts.next().map(|d| d.parse::<u64>().map_err(|_| format!("{{{n}}}: the salt must be a number"))).transpose()?.unwrap_or(0);
            if !(1..=24).contains(&digits) {
                return Err(format!("{{{n}}}: use 1 to 24 digits"));
            }
            Var::Number { digits, salt }
        }
        k if keys.contains(&k) => Var::Field(k.to_string()),
        other => {
            let known: Vec<String> = ["body", "today", "total", "number:6"].iter().map(|s| s.to_string()).chain(keys.iter().map(|k| k.to_string())).collect();
            return Err(format!("unknown {{{other}}} (use {})", known.join(", ")));
        }
    };
    Ok(Piece::Var(var, upper))
}

fn value(var: &Var, v: &Vars) -> String {
    let c = v.ctx;
    match var {
        Var::Body => c.body.trim().to_string(),
        Var::Today => today(),
        Var::Line => v.line.unwrap_or("").to_string(),
        Var::Name => split_price(v.line.unwrap_or("")).0,
        Var::Price => split_price(v.line.unwrap_or("")).1.map(money).unwrap_or_default(),
        Var::Index => (v.index + 1).to_string(),
        Var::Total => money(c.lines().iter().filter_map(|l| split_price(l).1).sum()),
        Var::Number { digits, salt } => c.number(0x7000 + salt, *digits),
        Var::Field(key) => {
            let s = c.field_in(v.fields, key);
            if s == "today" { today() } else { s }
        }
    }
}

// ---- reading a file ----------------------------------------------------------------------------

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn n(v: &Value, key: &str) -> Option<f64> {
    v.get(key)?.as_f64()
}

fn req_i(v: &Value, key: &str) -> Result<i32, String> {
    n(v, key).map(|x| x.round() as i32).ok_or_else(|| format!("needs a number \"{key}\""))
}

pub(super) fn font_of(name: Option<&str>) -> Result<FontChoice, String> {
    Ok(match name.map(|f| f.to_ascii_lowercase().replace([' ', '_'], "-")).as_deref() {
        None | Some("sans") => FontChoice::Sans,
        Some("bold") => FontChoice::Bold,
        Some("mono") => FontChoice::Mono,
        Some("serif") => FontChoice::Serif,
        Some("script") => FontChoice::Script,
        Some("old-english") => FontChoice::OldEnglish,
        Some("stencil") => FontChoice::Stencil,
        Some("impact") => FontChoice::Impact,
        Some("playbill") => FontChoice::Playbill,
        Some("typewriter") => FontChoice::Typewriter,
        Some("chancery") => FontChoice::Chancery,
        Some("papyrus") => FontChoice::Papyrus,
        Some("ocr") => FontChoice::Ocr,
        Some("marker") => FontChoice::Marker,
        Some("symbols") => FontChoice::Symbols,
        Some(other) => return Err(format!("unknown font '{other}' (sans, bold, mono, serif, script, old-english, stencil, impact, playbill, typewriter, chancery, papyrus, ocr, marker, symbols)")),
    })
}

fn style(v: &Value) -> Result<Style, String> {
    let align = match s(v, "align").map(str::to_ascii_lowercase).as_deref() {
        None | Some("left") => Align::Left,
        Some("center" | "centre") => Align::Center,
        Some("right") => Align::Right,
        Some(other) => return Err(format!("unknown align '{other}' (left, center, right)")),
    };
    Ok(Style { font: font_of(s(v, "font"))?, size: n(v, "size").unwrap_or(28.0).clamp(8.0, 240.0) as f32, align })
}

fn dash(v: &Value) -> (usize, usize) {
    match v.get("dash").and_then(Value::as_array) {
        Some(a) if a.len() == 2 => (a[0].as_u64().unwrap_or(0) as usize, a[1].as_u64().unwrap_or(0) as usize),
        _ => (0, 0),
    }
}

fn count(v: &Value, key: &str, default: usize, max: usize) -> usize {
    (n(v, key).unwrap_or(default as f64).max(0.0) as usize).min(max)
}

fn page_step(v: &Value, keys: &[&str], in_each: bool) -> Result<Placed, String> {
    let op = s(v, "op").or_else(|| s(v, "kind")).ok_or("needs an \"op\"")?;
    let tpl = |key: &str| -> Result<Tpl, String> {
        let src = s(v, key).ok_or_else(|| format!("\"{op}\" needs \"{key}\""))?;
        Tpl::parse(src, keys, in_each)
    };
    let step = match op {
        "text" => Step::Text { text: tpl("text")?, st: style(v)?, line_gap: n(v, "line_gap").unwrap_or(0.0) as f32, any: v.get("break_anywhere").and_then(Value::as_bool).unwrap_or(false) },
        "band" => Step::Band { text: tpl("text")?, st: style(v)? },
        "shout" => Step::Shout { text: tpl("text")?, font: font_of(s(v, "font"))?, max: n(v, "size").unwrap_or(120.0).clamp(12.0, 300.0) as f32 },
        "leader" => Step::Leader { left: tpl("left")?, right: tpl("right")?, font: font_of(s(v, "font"))?, size: n(v, "size").unwrap_or(26.0).clamp(8.0, 120.0) as f32 },
        "rule" => Step::Rule { dash: dash(v), thick: count(v, "thick", 2, 20), space: count(v, "space", 8, 100) },
        "flourish" => Step::Flourish,
        "gap" => Step::Gap(count(v, "rows", 10, 400)),
        "barcode" => Step::Barcode { data: tpl("data")?, rows: count(v, "rows", 60, 200).max(8) },
        "code" => Step::Code {
            data: tpl("data")?,
            kind: match s(v, "kind").map(str::to_ascii_lowercase).as_deref() {
                None | Some("qr") => CodeKind::Qr,
                Some("code128") => CodeKind::Code128,
                Some("code39") => CodeKind::Code39,
                Some("ean13") => CodeKind::Ean13,
                Some(other) => return Err(format!("unknown code kind '{other}' (qr, code128, code39, ean13)")),
            },
        },
        "each" => {
            if in_each {
                return Err("\"each\" cannot be nested".into());
            }
            Step::Each(page_steps(v.get("steps"), keys, true)?)
        }
        other => return Err(format!("unknown op '{other}' (text, band, shout, leader, rule, flourish, gap, barcode, code, each)")),
    };
    let region = if s(v, "region").is_some() {
        if op == "each" {
            return Err("\"each\" cannot carry a \"region\" (put it on the step that draws the text)".into());
        }
        if in_each {
            return Err("\"region\" cannot be used inside an \"each\" step (it repeats for every line; mark a step outside it instead)".into());
        }
        region_key(v, keys)?
    } else {
        None
    };
    Ok(Placed { step, region })
}

/// The `"region"` key of a step: the field it is the writable part of. Only fields the theme
/// declares may be named.
fn region_key(v: &Value, keys: &[&str]) -> Result<Option<String>, String> {
    let Some(key) = s(v, "region") else { return Ok(None) };
    if !keys.contains(&key) {
        let known = if keys.is_empty() { "this theme declares no fields".to_string() } else { format!("use one of: {}", keys.join(", ")) };
        return Err(format!("unknown region '{key}' ({known})"));
    }
    Ok(Some(key.to_string()))
}

fn page_steps(list: Option<&Value>, keys: &[&str], in_each: bool) -> Result<Vec<Placed>, String> {
    let list = list.and_then(Value::as_array).ok_or("needs a \"steps\" list")?;
    list.iter().enumerate().map(|(i, v)| page_step(v, keys, in_each).map_err(|e| format!("step {}: {e}", i + 1))).collect()
}

fn finish_step(v: &Value) -> Result<Finish, String> {
    Ok(match s(v, "op").ok_or("needs an \"op\"")? {
        "border" => Finish::Border(count(v, "inset", 6, 60)),
        "speckle" => Finish::Speckle { count: count(v, "count", 120, 5000), big: count(v, "big", 6, 100) },
        "ragged" => Finish::Ragged(count(v, "max", 6, 30)),
        "zigzag" => Finish::Zigzag(count(v, "tooth", 8, 40).max(2)),
        "tractor" => Finish::Tractor(count(v, "pitch", 40, 200).max(12)),
        other => return Err(format!("unknown finish '{other}' (border, speckle, ragged, zigzag, tractor)")),
    })
}

fn sheet_op(v: &Value, keys: &[&str]) -> Result<Op, String> {
    let op = s(v, "op").ok_or("needs an \"op\"")?;
    let tpl = |key: &str| -> Result<Tpl, String> {
        let src = s(v, key).ok_or_else(|| format!("\"{op}\" needs \"{key}\""))?;
        Tpl::parse(src, keys, false)
    };
    let t = n(v, "thick").unwrap_or(1.0).round().max(1.0) as i32;
    if op != "text" && s(v, "region").is_some() {
        return Err("\"region\" only works on a \"text\" step".into());
    }
    Ok(match op {
        "rect" => Op::Rect { x0: req_i(v, "x0")?, y0: req_i(v, "y0")?, x1: req_i(v, "x1")?, y1: req_i(v, "y1")?, t },
        "fill" => Op::Fill { x0: req_i(v, "x0")?, y0: req_i(v, "y0")?, x1: req_i(v, "x1")?, y1: req_i(v, "y1")? },
        "hline" => Op::HLine { y: req_i(v, "y")?, x0: req_i(v, "x0")?, x1: req_i(v, "x1")?, t, dash: dash(v) },
        "vline" => Op::VLine { x: req_i(v, "x")?, y0: req_i(v, "y0")?, y1: req_i(v, "y1")?, t, dash: dash(v) },
        "line" => Op::Line { x0: req_i(v, "x0")?, y0: req_i(v, "y0")?, x1: req_i(v, "x1")?, y1: req_i(v, "y1")?, t },
        "text" => {
            let anchor = match s(v, "anchor").map(str::to_ascii_lowercase).as_deref() {
                None | Some("left") => Anchor::Left,
                Some("center" | "centre") => Anchor::Centre,
                Some("right") => Anchor::Right,
                Some(other) => return Err(format!("unknown anchor '{other}' (left, center, right)")),
            };
            let st = style(v)?;
            Op::Text { text: tpl("text")?, font: st.font, size: st.size, x: req_i(v, "x")?, y: req_i(v, "y")?, anchor, knock: v.get("knockout").and_then(Value::as_bool).unwrap_or(false), region: region_key(v, keys)? }
        }
        "bars" => Op::Bars { data: tpl("data")?, x: req_i(v, "x")?, y: req_i(v, "y")?, h: req_i(v, "h")?.max(1), module: count(v, "module", 1, 6).max(1) },
        "punch" => Op::Punch { x: req_i(v, "x")?, y: req_i(v, "y")?, r: req_i(v, "r")?.max(1) },
        "disc" => Op::Disc { x: req_i(v, "x")?, y: req_i(v, "y")?, r: req_i(v, "r")?.max(1), ring: n(v, "ring").map(|r| r.round() as i32) },
        "round" => Op::Round(count(v, "radius", 14, 60)),
        other => return Err(format!("unknown op '{other}' (rect, fill, hline, vline, line, text, bars, punch, disc, round)")),
    })
}

fn script_steps(script: &str) -> Result<Value, String> {
    // Standard library for strings/arrays/maps/math, but no extra packages: no filesystem, network, OS or process APIs.
    let mut engine = Engine::new();
    engine.set_max_operations(100_000);
    let result: Dynamic = engine
        .eval_with_scope(&mut Scope::new(), script)
        .map_err(|e| format!("script error: {e}"))?;
    let value: Value =
        rhai::serde::from_dynamic(&result).map_err(|e| format!("script must return steps: {e}"))?;
    if !value.is_array() {
        return Err("script must return a list of steps".into());
    }
    Ok(value)
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn clean_id(raw: &str) -> Result<String, String> {
    let id: String = raw.trim().to_ascii_lowercase().chars().map(|c| if c == ' ' || c == '_' { '-' } else { c }).collect();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(format!("the id '{raw}' may only use letters, digits and dashes"));
    }
    Ok(id)
}

fn family_of(name: Option<&str>) -> Family {
    match name.map(|f| f.to_ascii_lowercase().replace([' ', '_'], "-")).as_deref() {
        Some("shop") => Family::Shop,
        Some("travel") => Family::Travel,
        Some("old-world" | "oldworld" | "parchment") => Family::OldWorld,
        Some("papers") => Family::Papers,
        Some("forms") => Family::Forms,
        Some("retro") => Family::Retro,
        _ => Family::Mine,
    }
}

/// Read one theme from the text of its file. Nothing is registered here.
pub fn parse(text: &str) -> Result<Custom, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| format!("not valid JSON ({e})"))?;
    if !root.is_object() {
        return Err("the file must hold one JSON object".into());
    }
    let id = clean_id(s(&root, "id").ok_or("needs an \"id\", the word used on the command line")?)?;
    let name = s(&root, "name").map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&id).to_string();
    let mut fields = Vec::new();
    for (i, f) in root.get("fields").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
        let key = s(f, "key").ok_or_else(|| format!("field {}: needs a \"key\"", i + 1))?;
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(format!("field {}: the key '{key}' may only use lowercase letters, digits and _", i + 1));
        }
        if fields.iter().any(|d: &Field| d.key == key) {
            return Err(format!("field {}: the key '{key}' is used twice", i + 1));
        }
        fields.push(Field { key: leak(key.to_string()), label: leak(s(f, "label").unwrap_or(key).to_string()), hint: leak(s(f, "hint").unwrap_or("").to_string()) });
    }
    if fields.len() > MAX_FIELDS {
        return Err(format!("{} fields is too many (at most {MAX_FIELDS})", fields.len()));
    }
    let keys: Vec<&str> = fields.iter().map(|f| f.key).collect();
    let layout = match s(&root, "layout").unwrap_or("page") {
        "page" => {
            let margin = n(&root, "margin").unwrap_or(24.0).clamp(0.0, 120.0) as f32;
            let steps_value = if let Some(script) = s(&root, "script") {
                script_steps(script)?
            } else {
                root.get("steps").cloned().ok_or("needs a \"steps\" list")?
            };
            let steps = page_steps(Some(&steps_value), &keys, false)?;
            let finish = match root.get("finish").and_then(Value::as_array) {
                None => Vec::new(),
                Some(list) => list.iter().enumerate().map(|(i, v)| finish_step(v).map_err(|e| format!("finish {}: {e}", i + 1))).collect::<Result<_, _>>()?,
            };
            Layout::Page { margin, steps, finish }
        }
        "sheet" => {
            let w = count(&root, "width", 780, 3000);
            let h = count(&root, "height", 384, 384);
            if w < 40 || h < 40 {
                return Err("a sheet needs a \"width\" of at least 40 and a \"height\" of 40 to 384".into());
            }
            let list = root.get("steps").and_then(Value::as_array).ok_or("needs a \"steps\" list")?;
            let ops = list.iter().enumerate().map(|(i, v)| sheet_op(v, &keys).map_err(|e| format!("step {}: {e}", i + 1))).collect::<Result<_, _>>()?;
            Layout::Sheet { w, h, ops }
        }
        other => return Err(format!("unknown layout '{other}' (page or sheet)")),
    };
    let body_optional = root.get("body_optional").and_then(Value::as_bool).unwrap_or(false);
    let sample = s(&root, "sample").map(str::to_string).unwrap_or_else(|| "Hello, world!".into());
    Ok(Custom {
        id: leak(id),
        name: leak(name),
        tagline: leak(s(&root, "tagline").map(str::trim).filter(|t| !t.is_empty()).unwrap_or("A theme of your own").to_string()),
        family: family_of(s(&root, "family")),
        fields: Box::leak(fields.into_boxed_slice()),
        sample: leak(sample),
        body_optional,
        layout,
    })
}

// ---- drawing -----------------------------------------------------------------------------------

fn u(v: i32) -> usize {
    v.max(0) as usize
}

fn run_steps(p: &mut Page, steps: &[Placed], v: &Vars) -> Result<(), String> {
    for Placed { step, region } in steps {
        if let Some(key) = region {
            p.begin(key);
        }
        match step {
            Step::Text { text, st, line_gap, any } => {
                let t = text.eval(v);
                if *any {
                    p.text_any(&t, st.font, st.size)?;
                } else if *line_gap != 0.0 {
                    p.text_gap(&t, st.font, st.size, st.align, *line_gap)?;
                } else {
                    p.text(&t, st.font, st.size, st.align)?;
                }
            }
            Step::Band { text, st } => p.band(&text.eval(v), st.font, st.size, st.align)?,
            Step::Shout { text, font, max } => p.shout(&text.eval(v), *font, *max)?,
            Step::Leader { left, right, font, size } => p.leader(&left.eval(v), &right.eval(v), *font, *size)?,
            Step::Rule { dash, thick, space } => p.rule(*dash, *thick, *space),
            Step::Flourish => p.flourish(),
            Step::Gap(rows) => p.gap(*rows),
            Step::Barcode { data, rows } => p.barcode(&data.eval(v), *rows)?,
            Step::Code { data, kind } => p.push(render::code(&data.eval(v), CodeOpts { kind: *kind, caption: false, ..CodeOpts::default() })?),
            Step::Each(inner) => {
                for (i, line) in v.ctx.lines().into_iter().enumerate() {
                    run_steps(p, inner, &Vars { ctx: v.ctx, fields: v.fields, line: Some(line), index: i })?;
                }
            }
        }
        if region.is_some() {
            p.end();
        }
    }
    Ok(())
}

fn run_ops(sheet: &mut Sheet, ops: &[Op], v: &Vars) -> Result<Vec<Region>, String> {
    let mut regions = Vec::new();
    for op in ops {
        match op {
            Op::Rect { x0, y0, x1, y1, t } => draw::rect(sheet, u(*x0), u(*y0), u(*x1), u(*y1), *t as usize),
            Op::Fill { x0, y0, x1, y1 } => draw::fill_rect(sheet, u(*x0), u(*y0), u(*x1), u(*y1)),
            Op::HLine { y, x0, x1, t, dash } => draw::hline(sheet, u(*y), u(*x0), u(*x1), *t as usize, *dash),
            Op::VLine { x, y0, y1, t, dash } => draw::vline(sheet, u(*x), u(*y0), u(*y1), *t as usize, *dash),
            Op::Line { x0, y0, x1, y1, t } => draw::line(sheet, *x0, *y0, *x1, *y1, *t),
            Op::Text { text, font, size, x, y, anchor, knock, region } => {
                let t = text.eval(v);
                let drawn = if *knock {
                    sheet.knockout(&t, *font, *size, *x, *y, *anchor)
                } else {
                    sheet.text(&t, *font, *size, *x, *y, *anchor)
                };
                if let (Some(key), Some((x0, y0, x1, y1))) = (region, drawn) {
                    let (fx0, fy0, fx1, fy1) = sheet.turned_box(x0, y0, x1, y1);
                    regions.push(Region { key: key.clone(), x0: fx0, x1: fx1 + 1, y0: fy0, y1: fy1 + 1 });
                }
            }
            Op::Bars { data, x, y, h, module } => {
                let modules = render::code128_modules(&data.eval(v)).map_err(|e| e.to_string())?;
                sheet.bars(&modules, *module, u(*x), u(*y), *h as usize);
            }
            Op::Punch { x, y, r } => draw::punch(sheet, *x, *y, *r),
            Op::Disc { x, y, r, ring } => draw::disc(sheet, *x, *y, *r, *ring),
            Op::Round(r) => {
                use draw::Canvas;
                let (w, h) = (sheet.width(), sheet.height());
                draw::round_corners(sheet, 0, 0, w - 1, h - 1, *r);
            }
        }
    }
    Ok(regions)
}

/// Draw `ctx` the way this theme's file says.
pub fn draw(c: &Custom, ctx: &Ctx) -> Result<Bitmap, String> {
    let v = Vars { ctx, fields: c.fields, line: None, index: 0 };
    match &c.layout {
        Layout::Page { margin, steps, finish } => {
            let mut p = Page::new(*margin);
            run_steps(&mut p, steps, &v)?;
            if p.height() == 0 {
                return Err("the theme drew nothing".into());
            }
            let mut b = p.done();
            let mut rng = ctx.rng(0x7001);
            for f in finish {
                match f {
                    Finish::Border(inset) => draw::ornate_border(&mut b, *inset),
                    Finish::Speckle { count, big } => draw::speckle(&mut b, &mut rng, *count, *big),
                    Finish::Ragged(max) => draw::ragged_edges(&mut b, &mut rng, *max),
                    Finish::Zigzag(tooth) => draw::zigzag_bottom(&mut b, *tooth),
                    Finish::Tractor(pitch) => draw::tractor_holes(&mut b, *pitch),
                }
            }
            Ok(b)
        }
        Layout::Sheet { w, h, ops } => {
            let mut sheet = Sheet::new(*w, *h);
            for region in run_ops(&mut sheet, ops, &v)? {
                super::mark(region);
            }
            Ok(sheet.turned())
        }
    }
}

// ---- the registry of loaded files ---------------------------------------------------------------

static REGISTRY: RwLock<Vec<&'static Custom>> = RwLock::new(Vec::new());
/// Files already turned into themes, by name and text, so a reload of an unchanged file reuses it.
static CACHE: Mutex<Option<HashMap<String, &'static Custom>>> = Mutex::new(None);

pub fn get(index: u16) -> Option<&'static Custom> {
    REGISTRY.read().ok()?.get(index as usize).copied()
}

pub fn count_loaded() -> usize {
    REGISTRY.read().map(|r| r.len()).unwrap_or(0)
}

/// `%APPDATA%\TronicPrinter\themes` (or the folder named by `TRONIC_THEMES_DIR`, to try themes
/// without touching the real one).
pub fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TRONIC_THEMES_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("TronicPrinter").join("themes"))
}

/// What reading the folder gave: how many themes, and one line per file that was refused.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub loaded: usize,
    pub errors: Vec<String>,
}

fn theme_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| !n.starts_with(['_', '.'])))
        .collect();
    files.sort();
    files
}

/// A number that changes when a theme file is added, removed or edited.
pub fn signature(dir: &Path) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in theme_files(dir) {
        p.file_name().hash(&mut h);
        if let Ok(m) = std::fs::metadata(&p) {
            m.len().hash(&mut h);
            m.modified().ok().hash(&mut h);
        }
    }
    h.finish()
}

/// Read every theme in `dir` without registering them.
pub fn scan(dir: &Path) -> (Vec<&'static Custom>, Vec<String>) {
    let mut themes: Vec<&'static Custom> = Vec::new();
    let mut errors = Vec::new();
    let mut seen: HashSet<String> = Theme::BUILTIN.iter().map(|t| t.id().to_string()).collect();
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    for path in theme_files(dir) {
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{file}: {e}"));
                continue;
            }
        };
        let key = format!("{file}\n{text}");
        let theme = match cache.get(&key) {
            Some(t) => *t,
            None => match parse(&text) {
                Ok(t) => {
                    let t: &'static Custom = Box::leak(Box::new(t));
                    cache.insert(key, t);
                    t
                }
                Err(e) => {
                    errors.push(format!("{file}: {e}"));
                    continue;
                }
            },
        };
        if !seen.insert(theme.id.to_string()) {
            errors.push(format!("{file}: the id '{}' is already taken by another theme", theme.id));
            continue;
        }
        themes.push(theme);
    }
    (themes, errors)
}

/// Replace the loaded themes with what is in `dir` now.
pub fn load_dir(dir: &Path) -> Report {
    let (themes, errors) = scan(dir);
    let loaded = themes.len();
    if let Ok(mut reg) = REGISTRY.write() {
        *reg = themes;
    }
    Report { loaded, errors }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WIDTH_DOTS;
    use crate::render::theme::Fields;

    fn ink(b: &Bitmap) -> u32 {
        b.data.iter().map(|x| x.count_ones()).sum()
    }

    fn draw_text(c: &Custom, body: &str, fields: &Fields) -> Result<Bitmap, String> {
        draw(c, &Ctx::new(body, fields))
    }

    const SHOP: &str = r#"{
        "id": "My Shop", "name": "My shop", "family": "shop", "sample": "Tea 1.00\nCake 3.50",
        "fields": [ {"key": "store", "label": "Store", "hint": "TEA ROOM"} ],
        "margin": 20,
        "steps": [
            {"op": "text", "text": "{store|upper}", "font": "bold", "size": 34, "align": "center"},
            {"op": "rule", "dash": [4, 4]},
            {"op": "each", "steps": [ {"op": "leader", "left": "{name}", "right": "{price}", "font": "mono", "size": 24} ]},
            {"op": "text", "text": "TOTAL {total}", "font": "bold", "size": 30, "align": "right"},
            {"op": "barcode", "data": "{number:8}", "rows": 40}
        ],
        "finish": [ {"op": "zigzag", "tooth": 8} ]
    }"#;

    #[test]
    fn a_page_theme_reads_and_draws_with_its_fields_and_lines() {
        let theme = parse(SHOP).unwrap();
        assert_eq!((theme.id, theme.name, theme.family, theme.fields.len()), ("my-shop", "My shop", Family::Shop, 1));
        let plain = draw_text(&theme, theme.sample, &Fields::new()).unwrap();
        assert!(plain.height > 150 && ink(&plain) > 500, "{} rows", plain.height);
        let named = draw_text(&theme, theme.sample, &Fields::from([("store".to_string(), "OTHER".to_string())])).unwrap();
        assert_ne!(plain, named, "the field is used");
        assert_eq!(plain, draw_text(&theme, theme.sample, &Fields::new()).unwrap(), "repeatable");
        let more = draw_text(&theme, "Tea 1.00\nCake 3.50\nJam 2.00", &Fields::new()).unwrap();
        assert!(more.height > plain.height, "one more line, one more row");
    }

    #[test]
    fn a_sheet_theme_is_drawn_sideways_and_turned() {
        let text = r#"{ "id": "tag", "layout": "sheet", "width": 500, "height": 300, "fields": [{"key":"to"}],
            "steps": [
              {"op": "rect", "x0": 0, "y0": 0, "x1": 499, "y1": 299, "thick": 3},
              {"op": "text", "text": "{to|upper}", "x": 250, "y": 100, "size": 60, "font": "impact", "anchor": "center"},
              {"op": "text", "text": "{body}", "x": 20, "y": 20, "size": 20, "knockout": true},
              {"op": "bars", "data": "{number:6}", "x": 20, "y": 240, "h": 30},
              {"op": "hline", "y": 200, "x0": 10, "x1": 490, "dash": [3, 3]},
              {"op": "punch", "x": 250, "y": 0, "r": 9}, {"op": "round", "radius": 12}
            ] }"#;
        let theme = parse(text).unwrap();
        let b = draw_text(&theme, "Hi", &Fields::from([("to".to_string(), "Lyon".to_string())])).unwrap();
        assert_eq!(b.height, 500, "as long as the sheet is wide");
        assert!(ink(&b) > 800);
    }

    // ---- writable regions ----------------------------------------------------------------------

    /// `render_mapped` reaches a theme through the folder registry; tests hand it over directly
    /// (`test_custom`) so they do not touch the registry other tests read.
    fn mapped(theme: Custom, body: &str, fields: &Fields) -> (Bitmap, Vec<Region>) {
        let theme: &'static Custom = Box::leak(Box::new(theme));
        crate::render::theme::test_custom(Some(theme));
        let out = crate::render::theme::render_mapped(Theme::Custom(0), body, fields, Default::default());
        crate::render::theme::test_custom(None);
        out.unwrap()
    }

    /// A page theme with no `"region"` key: the fields must still be found by looking at what is drawn.
    #[test]
    fn a_page_theme_already_says_where_its_fields_are() {
        let theme = parse(SHOP).unwrap();
        let (b, regions) = mapped(theme, "Tea 1.00\nCake 3.50", &Fields::new());
        let store = regions.iter().find(|r| r.key == "store").expect("the store field is found without any region key");
        assert_eq!((store.x0, store.x1), (20, WIDTH_DOTS - 20), "between the margins");
        assert_eq!(store.y0, 0, "its step is the first one");
        assert!(store.y0 < store.y1 && store.y1 <= b.height, "{store:?} in {} rows", b.height);
    }

    /// A `"region"` key on a page step marks its rows even when the text would not be recognised.
    #[test]
    fn a_page_step_can_say_which_field_it_is_the_place_for() {
        let text = r#"{ "id": "marked", "margin": 30, "fields": [{"key": "room"}],
            "steps": [
              {"op": "gap", "rows": 10},
              {"op": "text", "text": "Written by hand", "region": "room"},
              {"op": "gap", "rows": 4}
            ] }"#;
        let (b, regions) = mapped(parse(text).unwrap(), "x", &Fields::new());
        let room = regions.iter().find(|r| r.key == "room").expect("the marked step is a region");
        assert_eq!((room.x0, room.x1), (30, WIDTH_DOTS - 30), "between the margins");
        assert_eq!(room.y0, 10, "after the first gap");
        assert!(room.y1 > room.y0 && room.y1 <= b.height, "{room:?} in {} rows", b.height);
    }

    /// A sheet `text` op with a `"region"` gives the box of the drawn text, already turned.
    #[test]
    fn a_sheet_text_can_say_which_field_it_is_the_place_for() {
        let text = r#"{ "id": "turned", "layout": "sheet", "width": 500, "height": 300, "fields": [{"key": "to"}],
            "steps": [
              {"op": "text", "text": "{to|upper}", "x": 250, "y": 100, "size": 60, "font": "impact", "anchor": "center", "region": "to"}
            ] }"#;
        let (b, regions) = mapped(parse(text).unwrap(), "x", &Fields::from([("to".to_string(), "Lyon".to_string())]));
        assert_eq!(regions.len(), 1, "{regions:?}");
        let r = &regions[0];
        assert_eq!(r.key, "to");
        assert!(r.x0 < r.x1 && r.x1 <= WIDTH_DOTS, "{r:?} across the paper");
        assert!(r.y0 < r.y1 && r.y1 <= b.height, "{r:?} down {} rows", b.height);
        let ink = (r.y0..r.y1).flat_map(|y| (r.x0..r.x1).map(move |x| (x, y))).filter(|&(x, y)| b.get(x, y)).count();
        assert!(ink > 0, "the region is where the text was drawn");
    }

    #[test]
    fn a_region_key_is_checked_where_it_is_written() {
        let bad = |json: &str| parse(json).err().unwrap_or_else(|| "no error".into());
        let field = r#""fields": [{"key": "room"}]"#;
        assert!(bad(&format!(r#"{{"id": "a", {field}, "steps": [{{"op": "text", "text": "x", "region": "nope"}}]}}"#)).contains("unknown region 'nope'"));
        assert!(bad(&format!(r#"{{"id": "a", {field}, "steps": [{{"op": "each", "steps": [], "region": "room"}}]}}"#)).contains("cannot carry"));
        assert!(bad(&format!(r#"{{"id": "a", {field}, "steps": [{{"op": "each", "steps": [{{"op": "text", "text": "x", "region": "room"}}]}}]}}"#)).contains("inside an \"each\""));
        assert!(bad(&format!(r#"{{"id": "a", "layout": "sheet", {field}, "steps": [{{"op": "rect", "x0": 0, "y0": 0, "x1": 10, "y1": 10, "region": "room"}}]}}"#)).contains("only works on a \"text\""));
        assert!(bad(&format!(r#"{{"id": "a", "layout": "sheet", {field}, "steps": [{{"op": "text", "text": "x", "x": 0, "y": 0, "region": "nope"}}]}}"#)).contains("unknown region 'nope'"));
        assert!(parse(&format!(r#"{{"id": "a", {field}, "steps": [{{"op": "text", "text": "x", "region": "room"}}]}}"#)).is_ok());
        assert!(parse(&format!(r#"{{"id": "a", "layout": "sheet", {field}, "steps": [{{"op": "text", "text": "x", "x": 0, "y": 0, "region": "room"}}]}}"#)).is_ok());
    }

    #[test]
    fn mistakes_are_explained_where_they_are() {
        let bad = |json: &str| parse(json).err().unwrap_or_else(|| "no error".into());
        assert!(bad("{ nope").contains("not valid JSON"));
        assert!(bad("[]").contains("one JSON object"));
        assert!(bad(r#"{"steps": []}"#).contains("\"id\""));
        assert!(bad(r#"{"id": "a b!", "steps": []}"#).contains("letters, digits and dashes"));
        assert!(bad(r#"{"id": "a"}"#).contains("\"steps\""));
        assert!(bad(r#"{"id": "a", "steps": [{"op": "wobble"}]}"#).contains("step 1: unknown op 'wobble'"));
        assert!(bad(r#"{"id": "a", "steps": [{"op": "text", "text": "{nope}"}]}"#).contains("unknown {nope}"));
        assert!(bad(r#"{"id": "a", "steps": [{"op": "text", "text": "{line}"}]}"#).contains("inside an \"each\""));
        assert!(bad(r#"{"id": "a", "steps": [{"op": "text", "text": "{open"}]}"#).contains("never closed"));
        assert!(bad(r#"{"id": "a", "steps": [{"op": "text", "text": "x", "font": "comic"}]}"#).contains("unknown font"));
        assert!(bad(r#"{"id": "a", "steps": [{"op": "each", "steps": [{"op": "each", "steps": []}]}]}"#).contains("cannot be nested"));
        assert!(bad(r#"{"id": "a", "layout": "cube", "steps": []}"#).contains("unknown layout"));
        let five: Vec<String> = (0..=MAX_FIELDS).map(|i| format!("{{\"key\": \"k{i}\"}}")).collect();
        assert!(bad(&format!(r#"{{"id": "a", "fields": [{}], "steps": []}}"#, five.join(","))).contains("too many"));
        assert!(bad(r#"{"id": "a", "layout": "sheet", "steps": [{"op": "rect", "x0": 1}]}"#).contains("needs a number"));
    }

    #[test]
    fn braces_can_be_written_literally() {
        let t = Tpl::parse("{{braces}} {body}", &[], false).unwrap();
        let fields = Fields::new();
        let ctx = Ctx::new("hi", &fields);
        assert_eq!(t.eval(&Vars { ctx: &ctx, fields: &[], line: None, index: 0 }), "{braces} hi");
    }

    #[test]
    fn a_folder_is_read_file_by_file_and_a_bad_one_does_not_stop_the_others() {
        let dir = std::env::temp_dir().join(format!("tronic-themes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.json"), r#"{"id": "zz-a", "steps": [{"op": "text", "text": "{body}"}]}"#).unwrap();
        std::fs::write(dir.join("b.json"), r#"{"id": "zz-b", "steps": [{"op": "text", "text": "{body}"}]}"#).unwrap();
        std::fs::write(dir.join("broken.json"), "{ nope").unwrap();
        std::fs::write(dir.join("clash.json"), r#"{"id": "receipt", "steps": [{"op": "gap", "rows": 4}]}"#).unwrap();
        std::fs::write(dir.join("twin.json"), r#"{"id": "zz-a", "steps": [{"op": "gap", "rows": 4}]}"#).unwrap();
        std::fs::write(dir.join("_skipped.json"), "{ nope").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignore me").unwrap();
        let before = signature(&dir);
        let (themes, errors) = scan(&dir);
        assert_eq!(themes.iter().map(|t| t.id).collect::<Vec<_>>(), ["zz-a", "zz-b"]);
        assert_eq!(errors.len(), 3, "{errors:?}");
        assert!(errors.iter().any(|e| e.starts_with("broken.json: not valid JSON")));
        assert!(errors.iter().any(|e| e.starts_with("clash.json") && e.contains("already taken")));
        assert!(errors.iter().any(|e| e.starts_with("twin.json") && e.contains("already taken")));
        // The same file read again is the same theme, not a new copy.
        assert!(std::ptr::eq(scan(&dir).0[0], themes[0]));
        // Deleting a file is how a theme is removed.
        std::fs::remove_file(dir.join("b.json")).unwrap();
        assert_ne!(signature(&dir), before);
        assert_eq!(scan(&dir).0.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
        let (none, no_errors) = scan(&dir);
        assert!(none.is_empty() && no_errors.is_empty(), "a missing folder is just empty");
    }

    /// The example files shipped in `themes/examples` must stay valid and draw something.
    /// `THEME_SHEETS=dir cargo test the_example_themes` also writes a PNG of each, to look at.
    #[test]
    fn the_example_themes_shipped_with_the_app_are_valid_and_draw() {
        let examples = [("tea-room", include_str!("../../../themes/examples/tea-room.json")), ("day-pass", include_str!("../../../themes/examples/day-pass.json"))];
        for (id, text) in examples {
            let theme = parse(text).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(theme.id, id);
            let b = draw_text(&theme, theme.sample, &Fields::new()).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(b.height > 100 && ink(&b) > 500, "{id} drew almost nothing");
            if let Ok(dir) = std::env::var("THEME_SHEETS") {
                std::fs::create_dir_all(&dir).unwrap();
                let img = image::GrayImage::from_fn(crate::protocol::WIDTH_DOTS as u32, b.height as u32, |x, y| image::Luma([if b.get(x as usize, y as usize) { 20 } else { 244 }]));
                img.save(format!("{dir}/custom-{id}.png")).unwrap();
            }
        }
    }

    #[test]
    fn a_handle_to_a_theme_that_is_gone_is_harmless() {
        let gone = Theme::Custom(60_000);
        assert_eq!((gone.id(), gone.fields().len(), gone.family()), ("custom", 0, Some(Family::Mine)));
        assert!(render::themed(gone, "x", &Fields::new(), Default::default()).is_err());
    }

    #[test]
    fn a_script_theme_builds_the_same_steps_as_json() {
        let json = r#"{
            "id": "script-json",
            "layout": "page",
            "steps": [
              {"op": "text", "text": "Ligne 0"},
              {"op": "text", "text": "Ligne 1"},
              {"op": "text", "text": "Ligne 2"}
            ]
        }"#;
        let scripted = r##"{
            "id": "script-dyn",
            "layout": "page",
            "script": "let out = []; let i = 0; while i < 3 { out.push(#{ op: \"text\", text: \"Ligne \" + i.to_string() }); i += 1; } out"
        }"##;
        let a = parse(json).unwrap();
        let b = parse(scripted).unwrap();
        let body = "hi";
        let fields = Fields::new();
        assert_eq!(draw_text(&a, body, &fields).unwrap(), draw_text(&b, body, &fields).unwrap());
    }

    #[test]
    fn a_script_that_loops_forever_is_stopped() {
        let text = r#"{
            "id": "loop",
            "layout": "page",
            "script": "loop { }"
        }"#;
        let err = parse(text).err().unwrap_or_else(|| "no error".into());
        assert!(err.contains("script error"), "{err}");
    }

    #[test]
    fn a_script_returning_the_wrong_type_is_reported_not_panicking() {
        let text = r#"{
            "id": "bad-ret",
            "layout": "page",
            "script": "42"
        }"#;
        let err = parse(text).err().unwrap_or_else(|| "no error".into());
        assert!(err.contains("script must return"), "{err}");
    }
}
