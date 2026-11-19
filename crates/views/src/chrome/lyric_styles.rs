//! The lyric styles other than Focus. Each puts the line being sung on its own and animates it
//! from the playback position alone, so pausing freezes it and seeking lands on the right frame.

use std::ops::Range;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    AnyElement, FontWeight, HighlightStyle, Hsla, Pixels, SharedString, StyledText, div, px,
    relative,
};
use music::LyricsLine;
use state::LyricStyle;

/// How long a line takes to come in.
const ENTER: Duration = Duration::from_millis(450);
/// How far a line rises while it comes in.
const RISE: Pixels = px(18.);
/// How long a word takes to drop into place in Word by word.
const POP: Duration = Duration::from_millis(380);
/// How many old messages the chat keeps on screen.
const CHAT_KEPT: usize = 6;
/// How long before a line the chat shows someone typing.
const TYPING: Duration = Duration::from_millis(1100);
/// How many lines one sheet of paper holds.
const PAGE: usize = 4;
/// The share of a line's time its words are spread over when the source gives none.
const SPREAD: f32 = 0.85;
/// The longest a line's words are spread over.
const SPREAD_MOST: Duration = Duration::from_secs(7);
/// The handwriting face of the Paper style, from `assets/fonts`.
const HANDWRITING: &str = "Caveat";
/// A serif face for the Cinematic style; the platform falls back to its own serif.
const SERIF: &str = "Georgia";
/// A monospaced face for the Typewriter style.
#[cfg(target_os = "windows")]
const MONO: &str = "Consolas";
#[cfg(target_os = "macos")]
const MONO: &str = "Menlo";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const MONO: &str = "DejaVu Sans Mono";
/// Where the first rule of the paper sits below its line.
const RULE_DROP: Pixels = px(8.);
/// How thick a rule of the paper is.
const RULE: Pixels = px(1.);
/// How thick the margin line of the paper is.
const MARGIN_LINE: Pixels = px(2.);
/// How high the typing dots bounce.
const DOT_LIFT: Pixels = px(3.);
/// How faded each older chat message is, newest first.
const CHAT_FADE: [f32; 6] = [1., 0.9, 0.62, 0.36, 0.18, 0.08];
/// A pin before a place a chat message names.
const PIN: &str = "📍 ";
/// Places that, named in a line, get a pin under the chat message.
const PLACES: [(&str, &str); 24] = [
    ("cartagena", "Cartagena"),
    ("barranquilla", "Barranquilla"),
    ("medellín", "Medellín"),
    ("bogotá", "Bogotá"),
    ("puerto rico", "Puerto Rico"),
    ("la habana", "La Habana"),
    ("santo domingo", "Santo Domingo"),
    ("méxico", "México"),
    ("buenos aires", "Buenos Aires"),
    ("madrid", "Madrid"),
    ("barcelona", "Barcelona"),
    ("parís", "París"),
    ("paris", "Paris"),
    ("london", "London"),
    ("tokyo", "Tokyo"),
    ("new york", "New York"),
    ("brooklyn", "Brooklyn"),
    ("hollywood", "Hollywood"),
    ("los angeles", "Los Angeles"),
    ("miami", "Miami"),
    ("las vegas", "Las Vegas"),
    ("chicago", "Chicago"),
    ("atlanta", "Atlanta"),
    ("malecón", "El Malecón"),
];

/// What a style needs to draw one frame.
pub(super) struct Frame<'a> {
    pub lines: &'a [LyricsLine],
    pub position: Duration,
    /// The size the sung line is set at.
    pub size: Pixels,
    pub theme: ui::Theme,
    /// The song's title, for the top of the sheet in Paper.
    pub title: SharedString,
}

/// A word with its time, either from the source or spread over the line.
struct Word {
    start: Duration,
    end: Duration,
    text: String,
}

/// Draws `frame` in `style`. Focus is drawn by the sheet, not here.
pub(super) fn render(style: LyricStyle, frame: &Frame) -> AnyElement {
    match style {
        LyricStyle::Focus | LyricStyle::Karaoke => karaoke(frame),
        LyricStyle::Cinematic => cinematic(frame),
        LyricStyle::WordByWord => word_by_word(frame),
        LyricStyle::Typewriter => typewriter(frame),
        LyricStyle::Chat => chat(frame),
        LyricStyle::Paper => paper(frame),
    }
}

/// The colour that marks what has been sung: the cover's, or the theme's accent.
fn lit(theme: &ui::Theme) -> Hsla {
    theme.tint.unwrap_or(theme.primary)
}

fn active(lines: &[LyricsLine], position: Duration) -> Option<usize> {
    lines.iter().rposition(|line| line.start <= position)
}

/// How far `position` is through `start..start + span`, from 0 to 1.
fn through(start: Duration, span: Duration, position: Duration) -> f32 {
    match position.checked_sub(start) {
        Some(past) => (past.as_secs_f32() / span.as_secs_f32().max(0.001)).min(1.),
        None => 0.,
    }
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// Overshoots a little before settling, for words that drop into place.
fn ease_back(t: f32) -> f32 {
    let c = 1.9;
    1. + (c + 1.) * (t - 1.).powi(3) + c * (t - 1.).powi(2)
}

/// The words of a line with their times. Few sources time each word, so otherwise the words
/// are spread over the line by their length.
fn words(lines: &[LyricsLine], index: usize) -> Vec<Word> {
    let line = &lines[index];
    if let Some(words) = line.words.as_ref().filter(|words| !words.is_empty()) {
        return words
            .iter()
            .map(|word| Word {
                start: word.start,
                end: word.end.max(word.start),
                text: word.text.clone(),
            })
            .collect();
    }
    let end = line
        .end
        .or_else(|| lines.get(index + 1).map(|next| next.start))
        .unwrap_or(line.start + Duration::from_secs(4))
        .max(line.start + Duration::from_millis(400));
    let span = (end - line.start).mul_f32(SPREAD).min(SPREAD_MOST);
    let parts: Vec<&str> = line.text.split_inclusive(' ').collect();
    let total: f32 = parts
        .iter()
        .map(|part| part.trim().chars().count() as f32 + 1.5)
        .sum();
    let mut at = line.start;
    parts
        .into_iter()
        .map(|part| {
            let length = span.mul_f32((part.trim().chars().count() as f32 + 1.5) / total.max(1.));
            let word = Word {
                start: at,
                end: at + length,
                text: part.to_owned(),
            };
            at += length;
            word
        })
        .collect()
}

/// The line as one text, with the part already sung coloured `lit`.
fn painted(words: &[Word], position: Duration, lit: Hsla) -> StyledText {
    let mut text = String::new();
    let mut sung: Vec<Range<usize>> = Vec::new();
    for word in words {
        let from = text.len();
        let letters = word.text.trim_end().chars().count();
        let done = (letters as f32 * through(word.start, word.end - word.start, position)).round()
            as usize;
        let cut = word
            .text
            .char_indices()
            .nth(done)
            .map_or(word.text.len(), |(at, _)| at);
        text.push_str(&word.text);
        if cut > 0 {
            sung.push(from..from + cut);
        }
    }
    let style = HighlightStyle {
        color: Some(lit),
        ..HighlightStyle::default()
    };
    StyledText::new(text).with_highlights(sung.into_iter().map(move |range| (range, style)))
}

/// A column that holds a style in the middle of the lyrics view.
fn stage() -> gpui::Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_4()
        .px_4()
        .text_center()
}

/// The dots that light up before the first line.
fn waiting(frame: &Frame) -> AnyElement {
    let first = frame
        .lines
        .first()
        .map_or(Duration::from_secs(1), |line| line.start);
    let lit_dots = (through(Duration::ZERO, first, frame.position) * 3.999) as usize;
    let dot = frame.size * 0.35;
    stage()
        .child(div().flex().gap_3().children((0..3).map(|k| {
            div()
                .size(dot)
                .rounded_full()
                .bg(lit(&frame.theme))
                .opacity(if k <= lit_dots { 1. } else { 0.25 })
        })))
        .into_any_element()
}

/// The line after `index`, small and faded, so the next words are never a surprise.
fn coming(frame: &Frame, index: usize) -> Option<gpui::Div> {
    let next = frame.lines.get(index + 1)?;
    Some(
        div()
            .text_size(frame.size * 0.6)
            .font_weight(FontWeight::BOLD)
            .text_color(frame.theme.muted_foreground)
            .child(next.text.clone()),
    )
}

// Karaoke: the line fills with colour as it is sung; the next one waits underneath.
fn karaoke(frame: &Frame) -> AnyElement {
    let Some(index) = active(frame.lines, frame.position) else {
        return waiting(frame);
    };
    let enter = ease_out(through(frame.lines[index].start, ENTER, frame.position));
    stage()
        .child(
            div()
                .relative()
                .top(RISE * (1. - enter))
                .opacity(enter)
                .text_size(frame.size * 1.1)
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(frame.theme.muted_foreground)
                .child(painted(
                    &words(frame.lines, index),
                    frame.position,
                    lit(&frame.theme),
                )),
        )
        .children(coming(frame, index))
        .into_any_element()
}

// Cinematic: one big line in italics, like a subtitle; it rises in and the last one rises out.
fn cinematic(frame: &Frame) -> AnyElement {
    let index = active(frame.lines, frame.position);
    let start = index.map_or(Duration::ZERO, |index| frame.lines[index].start);
    let enter = ease_out(through(start, ENTER * 2, frame.position));
    let line = |text: String, shown: f32, offset: Pixels| {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .px_6()
            .child(
                div()
                    .relative()
                    .top(offset)
                    .opacity(shown)
                    .font_family(SERIF)
                    .italic()
                    .text_size(frame.size * 1.25)
                    .text_color(frame.theme.foreground)
                    .text_center()
                    .child(text),
            )
    };
    let current = index.map_or_else(|| "♪".to_owned(), |index| frame.lines[index].text.clone());
    div()
        .relative()
        .size_full()
        .when_some(
            index.and_then(|index| index.checked_sub(1)),
            |this, previous| {
                this.child(line(
                    frame.lines[previous].text.clone(),
                    1. - enter,
                    -RISE * 2. * enter,
                ))
            },
        )
        .child(line(current, enter, RISE * 2. * (1. - enter)))
        .into_any_element()
}

// Word by word: each word drops in with a little bounce right when it is sung.
fn word_by_word(frame: &Frame) -> AnyElement {
    let Some(index) = active(frame.lines, frame.position) else {
        return waiting(frame);
    };
    let accent = lit(&frame.theme);
    let size = frame.size * 1.15;
    stage()
        .child(
            div()
                .flex()
                .flex_wrap()
                .justify_center()
                .gap_x(size * 0.3)
                .children(words(frame.lines, index).into_iter().map(|word| {
                    let t = through(word.start, POP, frame.position);
                    let drop = ease_back(t);
                    let now = frame.position >= word.start && frame.position < word.end;
                    div()
                        .relative()
                        .top(RISE * (1. - drop))
                        .opacity(t.min(1.) * 1.5)
                        .text_size(size)
                        .font_weight(FontWeight::EXTRA_BOLD)
                        .text_color(if now { accent } else { frame.theme.foreground })
                        .child(word.text.trim().to_owned())
                })),
        )
        .children(coming(frame, index))
        .into_any_element()
}

// Typewriter: the line is typed letter by letter as it is sung, with a blinking caret.
fn typewriter(frame: &Frame) -> AnyElement {
    let Some(index) = active(frame.lines, frame.position) else {
        return waiting(frame);
    };
    let mut typed = String::new();
    for word in words(frame.lines, index) {
        let letters = word.text.chars().count();
        let shown = (letters as f32 * through(word.start, word.end - word.start, frame.position))
            .ceil() as usize;
        typed.extend(word.text.chars().take(shown));
        if shown < letters {
            break;
        }
    }
    let blink = (frame.position.as_millis() / 500).is_multiple_of(2);
    stage()
        .font_family(MONO)
        .when_some(index.checked_sub(1), |this, previous| {
            this.child(
                div()
                    .text_size(frame.size * 0.55)
                    .text_color(frame.theme.muted_foreground)
                    .child(frame.lines[previous].text.clone()),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .text_size(frame.size * 0.95)
                .text_color(frame.theme.foreground)
                .child(typed.trim_end().to_owned())
                .child(
                    div()
                        .w(frame.size * 0.5)
                        .h(frame.size)
                        .ml_1()
                        .bg(lit(&frame.theme))
                        .opacity(if blink { 1. } else { 0. }),
                ),
        )
        .into_any_element()
}

/// The lines that repeat in the song, which is nearly always the chorus.
fn chorus(lines: &[LyricsLine]) -> Vec<bool> {
    let key = |line: &LyricsLine| line.text.trim().to_lowercase();
    lines
        .iter()
        .map(|line| {
            let text = key(line);
            !text.is_empty() && lines.iter().filter(|other| key(other) == text).count() >= 2
        })
        .collect()
}

fn place(text: &str) -> Option<&'static str> {
    let lower = format!(" {} ", text.to_lowercase());
    PLACES.iter().find_map(|(word, name)| {
        let at = lower.find(word)?;
        let before = lower[..at].chars().next_back()?;
        let after = lower[at + word.len()..].chars().next()?;
        (!before.is_alphabetic() && !after.is_alphabetic()).then_some(*name)
    })
}

// Chat: lines arrive as messages; someone types first, old ones float up and fade, and the
// chorus comes from the other side.
fn chat(frame: &Frame) -> AnyElement {
    let theme = frame.theme;
    let sides = chorus(frame.lines);
    let shown = active(frame.lines, frame.position).map_or(0, |index| index + 1);
    let first = shown.saturating_sub(CHAT_KEPT);
    let typing = frame.lines.get(shown).is_some_and(|next| {
        !sides[shown] && next.start > frame.position && next.start - frame.position <= TYPING
    });
    let text_size = frame.size * 0.62;
    let bubble = |out: bool| {
        div()
            .max_w(relative(0.8))
            .px(text_size * 0.8)
            .py(text_size * 0.5)
            .rounded(theme.radius * 2.)
            .when(out, |this| this.rounded_br(theme.radius * 0.4))
            .when(!out, |this| this.rounded_bl(theme.radius * 0.4))
            .bg(if out { lit(&theme) } else { theme.secondary })
            .text_color(if out {
                theme.primary_foreground
            } else {
                theme.foreground
            })
            .text_size(text_size)
    };
    let row = |out: bool| {
        div()
            .w_full()
            .flex()
            .when(out, |this| this.justify_end())
            .when(!out, |this| this.justify_start())
    };
    let mut messages: Vec<AnyElement> = (first..shown)
        .map(|j| {
            let out = sides[j];
            let age = shown - 1 - j;
            let enter = ease_back(through(frame.lines[j].start, ENTER, frame.position).min(1.));
            let fade = CHAT_FADE[age.min(CHAT_FADE.len() - 1)];
            let side = if out { 1. } else { -1. };
            let lit_words = match j + 1 == shown {
                true => painted(&words(frame.lines, j), frame.position, theme.foreground),
                false => StyledText::new(frame.lines[j].text.clone()),
            };
            row(out)
                .child(
                    bubble(out)
                        .relative()
                        .left(RISE * 2. * side * (1. - enter))
                        .top(RISE * (1. - enter))
                        .opacity(fade * enter.clamp(0., 1.))
                        .flex()
                        .flex_col()
                        .child(lit_words)
                        .when_some(place(&frame.lines[j].text), |this, name| {
                            this.child(
                                div()
                                    .text_size(text_size * 0.7)
                                    .opacity(0.8)
                                    .child(format!("{PIN}{name}")),
                            )
                        }),
                )
                .into_any_element()
        })
        .collect();
    if typing {
        let beat = frame.position.as_secs_f32() * 6.;
        messages.push(
            row(false)
                .child(bubble(false).flex().gap_1().children((0..3).map(|k| {
                    let lift = ((beat - k as f32 * 0.9).sin() + 1.) / 2.;
                    div()
                        .relative()
                        .top(-DOT_LIFT * lift)
                        .size(text_size * 0.4)
                        .rounded_full()
                        .bg(theme.foreground)
                        .opacity(0.5 + 0.5 * lift)
                })))
                .into_any_element(),
        );
    }
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .w_full()
                .max_w(text_size * 26.)
                .h(relative(0.72))
                .flex()
                .flex_col()
                .justify_end()
                .gap_2()
                .px_4()
                .overflow_hidden()
                .children(messages),
        )
        .into_any_element()
}

// Paper: the lyrics handwritten on a sheet of lined paper as they are sung; the chorus gets
// highlighted and the sheet turns every few lines.
fn paper(frame: &Frame) -> AnyElement {
    let theme = frame.theme;
    let index = active(frame.lines, frame.position);
    let page = index.unwrap_or(0) / PAGE;
    let sides = chorus(frame.lines);
    let line_height = frame.size * 1.45;
    let first = page * PAGE;
    let turn = ease_out(through(
        frame
            .lines
            .get(first)
            .map_or(Duration::ZERO, |line| line.start),
        ENTER,
        frame.position,
    ));
    let marker = lit(&theme).opacity(0.35);
    let rows = (first..(first + PAGE).min(frame.lines.len())).map(|j| {
        let line_words = words(frame.lines, j);
        div()
            .h(line_height)
            .flex()
            .items_end()
            .gap_x(frame.size * 0.28)
            .overflow_hidden()
            .children(line_words.into_iter().map(|word| {
                let p = match index {
                    Some(at) if j < at => 1.,
                    Some(at) if j == at => {
                        through(word.start, word.end - word.start, frame.position)
                    }
                    _ => 0.,
                };
                let letters = word.text.trim().chars().count();
                let shown: String = word
                    .text
                    .trim()
                    .chars()
                    .take((letters as f32 * p).ceil() as usize)
                    .collect();
                div()
                    .when(sides[j] && p >= 1., |this| this.bg(marker))
                    .child(shown)
            }))
    });
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .child(
            div()
                .relative()
                .top(RISE * 3. * (1. - turn))
                .opacity(turn)
                .w_full()
                .max_w(frame.size * 22.)
                .pl(frame.size * 2.2)
                .pr_4()
                .py_2()
                .rounded(theme.radius * 0.5)
                .shadow_lg()
                .bg(theme.popover)
                .text_color(theme.popover_foreground)
                .font_family(HANDWRITING)
                .text_size(frame.size * 1.05)
                // the rules of the sheet and its margin
                .children((1..=PAGE + 1).map(|k| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(line_height * k as f32 + RULE_DROP)
                        .h(RULE)
                        .bg(theme.border)
                }))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(frame.size * 1.6)
                        .w(MARGIN_LINE)
                        .bg(theme.danger.opacity(0.45)),
                )
                .child(
                    div()
                        .h(line_height)
                        .flex()
                        .items_end()
                        .text_size(frame.size * 0.85)
                        .underline()
                        .child(frame.title.clone()),
                )
                .children(rows),
        )
        .into_any_element()
}
