use gpui::accesskit::Role;
use gpui::prelude::*;
use gpui::{Context, Entity, Pixels, SharedString, Subscription, Window, div, px, size};
use i18n::t;
use state::{Lyrics, LyricsState, Playback, Sonora};
use ui::{ActiveTheme as _, Artwork, Text};

use crate::shared::transport::transport;
use crate::shared::visualizer::VisualizerDrive;

/// How wide the dock is, closed or open, so it only ever grows downwards from where it sits.
pub const DOCK_WIDTH: Pixels = px(360.);
/// How tall the closed dock is: the cover, the line that plays and the bars.
pub const DOCK_CLOSED: Pixels = px(48.);
/// How tall the open dock is, with the controls and three lines of lyrics.
pub const DOCK_OPEN: Pixels = px(206.);
const COVER_SMALL: Pixels = px(32.);
const COVER_LARGE: Pixels = px(56.);
const BAR_WIDTH: Pixels = px(3.);
const BAR_HEIGHT: f32 = 16.;
const BAR_GAP: Pixels = px(2.);
/// How many bars the closed dock shows, drawn from the spectrum's lowest bands.
const BARS: usize = 4;

/// The little bar at the top of the desktop that shows the line playing while TK-Music is in the
/// background. A click opens it, with the cover, the controls and three lines of lyrics.
pub struct DesktopDock {
    playback: Entity<Playback>,
    lyrics: Entity<Lyrics>,
    visualizer: VisualizerDrive,
    open: bool,
    _observers: [Subscription; 2],
}

impl DesktopDock {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let sonora = Sonora::global(cx);
        let (playback, lyrics) = (sonora.playback.clone(), sonora.lyrics.clone());
        let _observers = [
            cx.observe(&playback, |_, _, cx| cx.notify()),
            cx.observe(&lyrics, |_, _, cx| cx.notify()),
        ];
        Self {
            playback,
            lyrics,
            visualizer: VisualizerDrive::default(),
            open: false,
            _observers,
        }
    }

    /// The lines before, at and after the one playing, empty without synced lyrics.
    fn lines(&self, cx: &Context<Self>) -> [Option<SharedString>; 3] {
        let lyrics = self.lyrics.read(cx);
        let Some(music::Lyrics::Synced { lines }) = lyrics
            .current()
            .filter(|_| matches!(lyrics.state(), LyricsState::Ready))
            .map(|hit| &hit.lyrics)
        else {
            return [None, None, None];
        };
        let at = self.playback.read(cx).live_position();
        let Some(index) = music::lyrics::active(lines, at) else {
            return [
                None,
                None,
                lines.first().map(|line| line.text.clone().into()),
            ];
        };
        let text = |index: Option<usize>| {
            index
                .and_then(|index| lines.get(index))
                .map(|line| line.text.trim())
                .filter(|text| !text.is_empty())
                .map(|text| SharedString::from(text.to_owned()))
        };
        [
            text(index.checked_sub(1)),
            text(Some(index)),
            text(Some(index + 1)),
        ]
    }
}

impl Render for DesktopDock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let playback = self.playback.read(cx);
        let track = playback.track().cloned();
        let playing = playback.wants_playing();
        match playing.then(|| playback.spectrum(cx)).flatten() {
            Some(spectrum) => self.visualizer.show(cx.entity_id(), spectrum, window),
            None => self.visualizer.hide(),
        }
        let [before, line, after] = self.lines(cx);
        let title: SharedString = track
            .as_ref()
            .map(|track| track.name.clone().into())
            .unwrap_or_default();
        let artists: SharedString = track
            .as_ref()
            .map(|track| track.artists.clone().into())
            .unwrap_or_default();
        let cover = track.as_ref().and_then(|track| track.cover.clone());
        let open = self.open;
        let bands = self.visualizer.levels().mixed();

        let bars = div()
            .flex()
            .items_end()
            .gap(BAR_GAP)
            .h(px(BAR_HEIGHT))
            .children((0..BARS).map(|bar| {
                let level = bands.get(bar * 2).copied().unwrap_or(0.).clamp(0.15, 1.);
                div()
                    .w(BAR_WIDTH)
                    .h(px(BAR_HEIGHT * level))
                    .rounded(BAR_WIDTH)
                    .bg(theme.primary)
            }));
        let dim = |text: Option<SharedString>| {
            div()
                .truncate()
                .text_size(theme.text(Text::Small))
                .text_color(theme.muted_foreground)
                .child(text.unwrap_or_default())
        };

        let body = match open {
            false => div()
                .flex()
                .items_center()
                .gap_3()
                .h_full()
                .px_2()
                .child(
                    Artwork::new(cover)
                        .size(COVER_SMALL)
                        .corner_radius(COVER_SMALL / 2.),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(line.unwrap_or(title)),
                )
                .child(bars)
                .into_any_element(),
            true => div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Artwork::new(cover)
                                .size(COVER_LARGE)
                                .corner_radius(theme.radius),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(title),
                                )
                                .child(dim(Some(artists))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .justify_center()
                        .child(transport(&self.playback, false, cx)),
                )
                .child(dim(before))
                .child(
                    div()
                        .truncate()
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(line.unwrap_or_else(|| "♪".into())),
                )
                .child(dim(after))
                .into_any_element(),
        };

        div()
            .id("desktop-dock")
            .role(Role::Group)
            .aria_label(t!("desktop-dock"))
            .size_full()
            .overflow_hidden()
            .rounded(match open {
                true => theme.radius * 2.,
                false => DOCK_CLOSED / 2.,
            })
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .text_color(theme.foreground)
            .text_size(theme.text(Text::Body))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| {
                this.open = !this.open;
                let height = match this.open {
                    true => DOCK_OPEN,
                    false => DOCK_CLOSED,
                };
                window.resize(size(DOCK_WIDTH, height));
                cx.notify();
            }))
            .child(body)
    }
}
