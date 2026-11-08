//! The ghost: the empty state's speech bubbles, and the face that floats round a ticket.

use rune::tok::type_;
use rune::{Color, Drag, El, Phase, Tx, col, el, memo, row, text};

use crate::app::App;
use crate::style;
use crate::voice;

use super::{MACHINE_W, STAGE_PAD};


/// The ghost's bubble above the slit, and the narrower one that stands beside the printer.
const GHOST_W: f32 = 340.0;
const GHOST_SIDE_W: f32 = 260.0;
/// The gap between the printer's case and the bubble beside it, and how far the bubble is
/// lifted off the floor (so its tail points at the case, not at the desk).
const GHOST_SIDE_GAP: f32 = 16.0;
const GHOST_SIDE_LIFT: f32 = 64.0;
/// The room the stage's inside needs to stand the bubble beside the printer without moving it:
/// the machine, both sides, and a last gap so the bubble never touches the stage's edge.
pub(crate) const GHOST_SIDE_MIN_W: f32 = MACHINE_W + 2.0 * (GHOST_SIDE_W + GHOST_SIDE_GAP) + GHOST_SIDE_GAP;

/// The ghost that floats round a ticket: a comic bubble over the paper face, one column. The
/// bubble is cream with a dark, round outline, a little tilted, and a tail that grows out of its
/// bottom edge at the face. Its size comes from its own words (measured in the label face), so the
/// talk can never be cut off: wrapped at `GHOST_TEXT_W`, at most `GHOST_TEXT_LINES` lines, then
/// shortened with an ellipsis. The column the placement keeps off the ticket is the bubble and
/// the face, `GHOST_TAIL_GAP` apart.
pub(crate) const GHOST_TEXT_W: f32 = 150.0;
const GHOST_TEXT_MIN_W: f32 = 84.0;
pub(crate) const GHOST_TEXT_LINES: usize = 3;
/// The room the card keeps round its talk.
pub(crate) const GHOST_PAD_X: f32 = 18.0;
pub(crate) const GHOST_PAD_Y: f32 = 12.0;
const GHOST_BUBBLE_MIN_W: f32 = 96.0;
const GHOST_BUBBLE_RADIUS: f32 = 18.0;
/// The card leans a hair, the way a bubble stuck on a page does.
const GHOST_TILT: f32 = -0.035;
/// How far the eyes slide toward the hand while a drag leans the column: at the limit, about half
/// a pupil. Small on purpose: it is a glance, not a swivel.
const GHOST_SWING_GAZE: f32 = 3.0;
pub(crate) const GHOST_BODY_W: f32 = 100.0;
pub(crate) const GHOST_BODY_H: f32 = 88.0;
/// Between the card's bottom edge and the face: room for the tail's point, not touching.
pub(crate) const GHOST_TAIL_GAP: f32 = 14.0;
/// The tail: a little square stood on a corner, half under the card.
const GHOST_TAIL: f32 = 14.0;
/// Clearance between the ghost and the paper, and between it and the stage's edge.
pub(crate) const GHOST_GAP: f32 = 12.0;
pub(crate) const GHOST_EDGE: f32 = 8.0;
/// How far the column's foot floats above the stage's floor when it keeps the case company.
const GHOST_LIFT: f32 = 48.0;

/// The ghost's face: a torn scrap of receipt paper, with ink-dot eyes that follow the pointer.
fn ghost_face(mood: voice::Mood, gaze: (f32, f32)) -> El {
    let paper = style::rgb(0xfaf6ec);
    let ink = style::rgb(0x241d16);
    let dot = |w: f32, h: f32| el().size(w, h).radius(h / 2.0).fill(ink);
    // An outlined eye with a pupil that slides toward the pointer, never past the rim.
    let eye = |size: f32| {
        let pupil = (size * 0.45).max(4.0);
        let room = size - pupil;
        let dx = gaze.0 * (room / 2.0 - 0.8);
        let dy = gaze.1 * (room / 2.0 - 0.8);
        el()
            .size(size, size)
            .radius(size / 2.0)
            .fill(paper)
            .border(1.5, ink)
            .child(el().size(pupil, pupil).radius(pupil / 2.0).fill(ink).absolute(room / 2.0 + dx, room / 2.0 + dy))
    };
    let eyes = match mood {
        voice::Mood::Asleep => row().gap(18.0).child(dot(12.0, 2.0)).child(dot(12.0, 2.0)),
        voice::Mood::Curious => row().gap(14.0).child(eye(13.0)).child(eye(9.0)),
        voice::Mood::Happy => row().gap(20.0).child(eye(12.0)).child(eye(12.0)),
    };
    // The mouth is the receipt's barcode: bars of different widths, hung along a curve so the
    // row reads as a smile (happy), a small "o" (curious) or a flat line (asleep).
    let bar_row = |widths: &[f32], drops: &[f32], h: f32| {
        widths.iter().zip(drops).fold(row().gap(1.5).h(h + 6.0), |r, (w, dy)| r.child(el().w(*w).h(h).fill(ink).offset(0.0, *dy)))
    };
    let mouth = match mood {
        voice::Mood::Happy => bar_row(
            &[2.0, 3.0, 1.5, 2.5, 2.0, 3.5, 1.5, 2.5, 2.0, 3.0, 1.5, 2.0],
            &[0.0, 0.5, 1.5, 2.5, 3.5, 4.0, 4.0, 3.5, 2.5, 1.5, 0.5, 0.0],
            7.0,
        ),
        voice::Mood::Curious => bar_row(&[2.0, 3.0, 1.5, 2.5, 2.0], &[0.0, 0.0, 0.0, 0.0, 0.0], 4.0),
        voice::Mood::Asleep => bar_row(&[2.0, 3.0, 1.5, 2.5, 2.0, 3.0, 1.5, 2.0], &[2.0; 8], 2.0),
    };
    // The torn top edge: uneven teeth (turned squares), half hidden behind the strip.
    let tears = [(11.0, 0.80), (8.0, 0.68), (12.0, 0.86), (9.0, 0.73), (10.0, 0.82), (7.0, 0.66), (11.0, 0.78), (9.0, 0.72), (10.0, 0.84)];
    let teeth = tears.iter().fold(row().gap(0.5), |r, (size, angle)| r.child(el().size(*size, *size).radius(1.5).fill(paper).rotate(*angle)));
    // The face lifts off the stage like the comic bubble does. Its paper is a torn stack, so the
    // shadow sits on the strip rather than the column: a column's shadow is one rectangle, and its
    // straight edges would draw across the teeth. Offset as deep as the blur, so none of it climbs
    // over the strip's top edge: the torn edges cast nothing, the straight sides and the foot do.
    let shade = Color::BLACK.alpha(0.30);
    let strip = col().items_center().gap(8.0).px(26.0).py(14.0).radius(0.0).fill(paper).shadow(3.0, 3.0, shade).child(eyes).child(mouth);
    // The same tear along the bottom, a little different so the two edges do not mirror.
    let teeth_bottom = tears.iter().rev().fold(row().gap(0.5), |r, (size, angle)| r.child(el().size(*size, *size).radius(1.5).fill(paper).rotate(*angle)));
    let face = col().items_center().child(teeth).child(strip.offset(0.0, -6.0)).child(teeth_bottom.offset(0.0, -12.0));
    if mood == voice::Mood::Asleep {
        row().gap(8.0).center_y().child(face).child(text("z Z".to_string()).font(type_::LABEL).color(Color::WHITE.alpha(0.4)))
    } else {
        face
    }
}

/// The empty state: the printer talks. A speech bubble over the slit with a little paper face and
/// a line that changes with the tab, bobbing gently (when effects are on). The real ticket takes
/// its place as soon as there is something to print.
pub(crate) fn ghost_paper(s: &App, gaze: (f32, f32)) -> El {
    // A picture held over the preview: the stage says "Let go to preview" itself, so the bubble steps aside.
    if s.over_stage && s.drag.as_ref().is_some_and(|d| d.travel >= crate::app::DRAG_MIN) {
        return col();
    }
    // The bubble's tail points down at the slit.
    let tail = el().w(14.0).h(14.0).radius(3.0).fill(glass()).rotate(0.785).offset(0.0, -9.0);
    col().items_center().child(ghost_bubble(s, gaze, GHOST_W)).child(tail)
}

/// The empty state beside the printer, used when the stage is wide enough to hold both: the bubble
/// stands to the right of the case, its tail pointing left at it, so the printer keeps the room its
/// ticket will need (it must not move when a ticket appears).
pub(crate) fn ghost_beside(s: &App, gaze: (f32, f32)) -> El {
    if s.over_stage && s.drag.as_ref().is_some_and(|d| d.travel >= crate::app::DRAG_MIN) {
        return col();
    }
    let stage_w = memo::layout_size("stage-frame").map_or(0.0, |r| r[0]);
    let left = (stage_w - 2.0 * STAGE_PAD) / 2.0 + MACHINE_W / 2.0 + GHOST_SIDE_GAP;
    // The tail is the bubble's left edge, tucked into its border.
    let tail = el().w(14.0).h(14.0).radius(3.0).fill(glass()).rotate(0.785).offset(6.0, 0.0);
    col()
        .pin(Some(left), None, None, Some(GHOST_SIDE_LIFT))
        .child(row().center_y().child(tail).child(ghost_bubble(s, gaze, GHOST_SIDE_W)))
}

/// The bubble itself: the face, a line of talk, and the tap that changes it. `width` lets the empty
/// state stand either above the slit (wide) or beside the printer (narrow).
fn ghost_bubble(s: &App, gaze: (f32, f32), width: f32) -> El {
    let (line, typing) = s.ghost_line_shown();
    let line = if typing { format!("{line}▌") } else { line };
    col()
        .items_center()
        .gap(8.0)
        .px(30.0)
        .py(16.0)
        .radius(20.0)
        .fill(glass())
        .border(1.0, Color::WHITE.alpha(0.16))
        .w(width)
        .child(ghost_face(s.ghost_mood(), gaze).key("ghost-face"))
        .child(col().min_h(44.0).center().child(text(line).font(type_::BODY).color(style::rgb(0xfaf6ec).alpha(0.85)).wrap().max_w(width - 50.0)))
        .on_press(|s: &mut App| s.ghost_poke())
}

/// The bubble's glass: a faint white pane, so it reads as frosted glass over the stage.
fn glass() -> Color {
    Color::WHITE.alpha(0.09)
}

/// What the floating ghost says, fitted to its comic bubble: the line as far as it has typed
/// out (with the cursor), wrapped at `GHOST_TEXT_W`, cut short with an ellipsis past
/// `GHOST_TEXT_LINES` lines, and the size of the card that holds it. The card is sized from the
/// measured text, so no word is ever clipped by it.
pub(crate) struct Talk {
    pub(crate) text: String,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

impl Talk {
    /// `max_w` is the widest the talk may run: the room the stage leaves beside the paper, up to
    /// `GHOST_TEXT_W`. A narrow window gets a taller card rather than a clipped or missing one.
    pub(crate) fn new(line: &str, typing: bool, max_w: f32) -> Talk {
        let style = type_::LABEL;
        let max_w = max_w.clamp(GHOST_TEXT_MIN_W, GHOST_TEXT_W);
        let shown = if typing { format!("{line}▌") } else { line.to_string() };
        // Three lines of this face, plus a point: the height the fitted talk may take.
        let line_h = rune::text::measure("Hg", &[], style, None).1;
        let text = ghost_fit(&shown, max_w, line_h * GHOST_TEXT_LINES as f32 + 1.0);
        let (tw, th) = rune::text::measure(&text, &[], style, Some(max_w));
        Talk {
            w: (tw + 2.0 * GHOST_PAD_X + 1.0).clamp(GHOST_BUBBLE_MIN_W, max_w + 2.0 * GHOST_PAD_X + 1.0).ceil(),
            h: (th + 2.0 * GHOST_PAD_Y).ceil() + 2.0,
            text,
        }
    }
}

/// `line` wrapped at `max_w` and cut to `room_h`, with an ellipsis where the words stop: whole
/// words only, and never more than the room allows. A line that fits comes back untouched.
fn ghost_fit(line: &str, max_w: f32, room_h: f32) -> String {
    let fits = |t: &str| rune::text::measure(t, &[], type_::LABEL, Some(max_w)).1 <= room_h;
    if fits(line) {
        return line.to_string();
    }
    // Cut the kept words in halves rather than one at a time: the measure is cached, but a long
    // line still costs a handful of layouts every time it changes.
    let chars: Vec<char> = line.chars().collect();
    let cut = |n: usize| {
        let mut kept: String = chars[..n].iter().collect();
        // Do not leave half a word behind: back to the last space, unless that leaves nothing.
        if let Some(space) = kept.rfind(' ') {
            if !kept[..space].trim().is_empty() {
                kept.truncate(space);
            }
        }
        format!("{}…", kept.trim_end())
    };
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if fits(&cut(mid)) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

/// The comic bubble the floating ghost talks through: cream paper, a round dark outline, a slight
/// lean and a tail that grows out of the card's bottom edge at the face.
fn ghost_comic(talk: &Talk) -> El {
    let paper = style::rgb(0xfaf6ec);
    let ink = style::rgb(0x1a1228);
    let (w, h) = (talk.w, talk.h);
    // The tail goes down first, so the card's fill hides the half it overlaps; the mask then rubs
    // out the stretch of outline the tail opens, and the join reads as one shape rather than a
    // diamond stuck on a card.
    let tail = el()
        .absolute((w - GHOST_TAIL) / 2.0, h - GHOST_TAIL + 4.0)
        .size(GHOST_TAIL, GHOST_TAIL)
        .radius(2.0)
        .fill(paper)
        .border(2.0, ink)
        .rotate(std::f32::consts::FRAC_PI_4);
    let mask = el().absolute((w - 24.0) / 2.0, h - 4.0).size(24.0, 5.0).fill(paper);
    col()
        .w(w)
        .h(h)
        .rotate(GHOST_TILT)
        .child(tail)
        .child(
            col()
                .absolute(0.0, 0.0)
                .w(w)
                .h(h)
                .px(GHOST_PAD_X)
                .py(GHOST_PAD_Y)
                .radius(GHOST_BUBBLE_RADIUS)
                .fill(paper)
                .border(2.0, ink)
                .shadow(2.0, 10.0, Color::BLACK.alpha(0.35))
                .justify_center()
                .child(text(talk.text.clone()).font(type_::LABEL).color(ink).wrap()),
        )
        .child(mask)
        .on_press(|s: &mut App| s.ghost_poke())
}

/// The ghost when a ticket takes the stage: the comic bubble and, under it, the paper face, bobbing
/// a few points and leaning a hair so it reads as flying. Off, the effects switch leaves it still;
/// the eyes keep following the pointer either way. A press on the bubble asks for another line and
/// another spot; the face is grabbed and dragged (see `ghost_grab` in `app`). `room` is the stage's
/// inside size, so the drag closure knows the bounds without a layout lookup at event time.
pub(crate) fn ghost_flying(
    s: &App,
    gaze: (f32, f32),
    talk: Option<&Talk>,
    at: (f32, f32),
    room: (f32, f32),
    stack: (f32, f32),
) -> El {
    let (bob, tilt) = if s.fx_on {
        let t = s.clock as f32;
        ((t * std::f32::consts::TAU / 3.2).sin() * 4.0, (t * std::f32::consts::TAU / 5.3).sin() * 0.02)
    } else {
        (0.0, 0.0)
    };
    // The lean a drag gives the column: it trails the hand, and `tick_ghost_swing` fades it once
    // the hand lets go. The eyes look the way the drag goes, which is the lean's other way round.
    let swing = s.ghost_swing;
    let gaze = ((gaze.0 - swing * GHOST_SWING_GAZE).clamp(-1.0, 1.0), gaze.1);
    // The hand that grabs the face: the press notes where the column stands, and every move puts
    // it back there plus the pointer's travel since the press. That travel is `d.dx`/`d.dy`, in
    // window points: the column's own `d.x`/`d.y` follows the ghost as it moves, so the pointer
    // sits at the same spot of it and a position read from there would never change.
    // `room` is the stage's inside size, captured at build time: `memo::layout` cannot be read
    // from an event (it answers only inside a view build, so it is `None` and swallows the move).
    let face = col()
        .w(GHOST_BODY_W)
        .h(GHOST_BODY_H)
        .center()
        .child(ghost_face(s.ghost_mood(), gaze).key("ghost-face").rotate(tilt))
        .cursor(rune::Cursor::Grab)
        .on_drag(move |s: &mut App, d: &mut Drag, _: &mut Tx| match d.phase {
            Phase::Down => s.ghost_grab(at),
            Phase::Move => {
                if let Some(from) = s.ghost_drag {
                    s.ghost_drag_to(dragged_to(from, (d.dx, d.dy), room, stack));
                }
            }
            Phase::Up => s.ghost_release(),
        });
    let mut stack_el = col().key("ghost-stack").w(stack.0).h(stack.1).items_center().offset(0.0, bob);
    if swing != 0.0 {
        // Pivot at the middle of the face at the foot of the column, so the bubble sways above a
        // face that keeps the spot `ghost_place` chose. A swing of zero adds nothing at all: the
        // still ghost is drawn exactly as before.
        let pivot = ((stack.1 - GHOST_BODY_H / 2.0) / stack.1.max(1.0)).clamp(0.0, 1.0);
        stack_el = stack_el.rotate(swing).origin(0.5, pivot);
    }
    if let Some(talk) = talk {
        stack_el = stack_el.gap(GHOST_TAIL_GAP).child(ghost_comic(talk));
    }
    stack_el.child(face)
}

/// Where the floating ghost stands: the whole column (bubble and face), the face alone when the
/// bubble has no room, or nothing when neither fits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GhostPlace {
    Whole((f32, f32)),
    Face((f32, f32)),
    Hidden,
}

/// Where the floating ghost stands in the stage's inside. `spot` is the one it picked; the others
/// are tried in turn, so a narrow stage gets the bubble on its other side rather than clipped by
/// the edge. A spot is first pulled inside the stage (a column that would cross an edge is clamped
/// to it); if that lands it on the ink, the next spot is tried. With no room for the column, only
/// the face is placed; with none for that either, nothing is drawn. `ticket` is the ticket's
/// rectangle and `case_right` the printer's right edge, in the stage's points, from its top-left.
pub(crate) fn ghost_place(spot: u8, inside: (f32, f32), stack: (f32, f32), ticket: (f32, f32, f32, f32), case_right: f32) -> GhostPlace {
    let (w, h) = inside;
    let (tx, ty, tw, th) = ticket;
    let (sw, sh) = stack;
    // Beside the ticket, the face keeps the paper company at the ticket's middle.
    let top = (ty + th / 2.0 - (sh - GHOST_BODY_H / 2.0)).clamp(GHOST_EDGE, (h - sh - GHOST_EDGE).max(GHOST_EDGE));
    let spots = [
        (tx + tw + GHOST_GAP, top),
        (tx - GHOST_GAP - sw, top),
        ((w - sw) / 2.0, ty - GHOST_GAP - sh),
        (case_right + GHOST_GAP, h - GHOST_LIFT - sh),
    ];
    let clamp = |(x, y): (f32, f32)| {
        (
            x.clamp(GHOST_EDGE, (w - sw - GHOST_EDGE).max(GHOST_EDGE)),
            y.clamp(GHOST_EDGE, (h - sh - GHOST_EDGE).max(GHOST_EDGE)),
        )
    };
    for k in 0..spots.len() {
        let at = clamp(spots[(spot as usize + k) % spots.len()]);
        if !covers(at, stack, ticket) {
            return GhostPlace::Whole(at);
        }
    }
    // No room for the bubble anywhere: the face on its own, somewhere clear of the ink.
    let (fw, fh) = (GHOST_BODY_W, GHOST_BODY_H);
    let face_clamp = |(x, y): (f32, f32)| {
        (
            x.clamp(GHOST_EDGE, (w - fw - GHOST_EDGE).max(GHOST_EDGE)),
            y.clamp(GHOST_EDGE, (h - fh - GHOST_EDGE).max(GHOST_EDGE)),
        )
    };
    for k in 0..spots.len() {
        let at = face_clamp(spots[(spot as usize + k) % spots.len()]);
        if !covers(at, (fw, fh), ticket) {
            return GhostPlace::Face(at);
        }
    }
    GhostPlace::Hidden
}

/// Whether the ghost's column (`stack` wide and high, at `(x, y)`) would sit over the ticket.
pub(crate) fn covers((x, y): (f32, f32), (sw, sh): (f32, f32), (tx, ty, tw, th): (f32, f32, f32, f32)) -> bool {
    x < tx + tw && tx < x + sw && y < ty + th && ty < y + sh
}

/// A place the user dragged the ghost to, pulled back inside the stage.
pub(crate) fn inside_stage((x, y): (f32, f32), (w, h): (f32, f32), (sw, sh): (f32, f32)) -> (f32, f32) {
    (
        x.clamp(GHOST_EDGE, (w - sw - GHOST_EDGE).max(GHOST_EDGE)),
        y.clamp(GHOST_EDGE, (h - sh - GHOST_EDGE).max(GHOST_EDGE)),
    )
}

/// Where the ghost's column goes while the hand holds it: the place it had when the button went
/// down (`from`) plus the pointer's travel since (`travel`, window points), kept inside the stage
/// (`stage` is the room it may use, `ghost` the column's own size).
pub(crate) fn dragged_to(from: (f32, f32), travel: (f32, f32), stage: (f32, f32), ghost: (f32, f32)) -> (f32, f32) {
    inside_stage((from.0 + travel.0, from.1 + travel.1), stage, ghost)
}

#[cfg(test)]
mod tests {
    use super::dragged_to;

    use crate::app::App;

    const STAGE: (f32, f32) = (900.0, 600.0);
    const GHOST: (f32, f32) = (260.0, 300.0);

    #[test]
    fn a_dragged_ghost_follows_the_hand_from_where_it_was_and_stops_at_the_edge() {
        let from = (300.0, 100.0);
        assert_eq!(dragged_to(from, (0.0, 0.0), STAGE, GHOST), from, "a press with no move leaves it where it was");
        assert_eq!(dragged_to(from, (120.0, 40.0), STAGE, GHOST), (420.0, 140.0), "the column takes the pointer's travel");
        // Past the top-left corner: it stops at the stage's edge, not outside it.
        assert_eq!(dragged_to(from, (-500.0, -400.0), STAGE, GHOST), (8.0, 8.0), "the corner holds it in");
        // Past the bottom-right corner: the column's own size is kept clear of the edge.
        let (x, y) = dragged_to(from, (5000.0, 5000.0), STAGE, GHOST);
        assert_eq!((x, y), (900.0 - 260.0 - 8.0, 600.0 - 300.0 - 8.0), "it stops with the column inside the stage");
    }

    #[test]
    fn a_press_a_move_and_a_release_carry_the_ghost_in_the_room_it_was_given() {
        // The path the face's drag closure takes, with the stage's inside passed in as `room`:
        // no layout lookup at event time, so the move is never silently dropped.
        let mut app = App::default();
        let from = (300.0, 100.0);
        app.ghost_grab(from);
        app.ghost_drag_to(dragged_to(from, (120.0, 40.0), STAGE, GHOST));
        assert_eq!(app.ghost_at, Some((420.0, 140.0)), "a move in the given room carries it");
        app.ghost_drag_to(dragged_to(from, (5000.0, 5000.0), STAGE, GHOST));
        assert_eq!(app.ghost_at, Some((632.0, 292.0)), "the given room's edge holds it");
        app.ghost_release();
        assert_eq!((app.ghost_drag, app.ghost_at), (None, Some((632.0, 292.0))), "letting go keeps the place");
    }
}
