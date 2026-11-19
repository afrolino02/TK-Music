use gpui::accesskit::Role;
use gpui::prelude::*;
use gpui::{Context, Entity, Pixels, SharedString, Subscription, Window, div, px};
use i18n::t;
use state::{Playback, Sonora};
use ui::{ActiveTheme as _, Artwork, Text, Visualizer, VisualizerStyle};

use crate::shared::transport::{next, previous, toggle};
use crate::shared::visualizer::VisualizerDrive;

/// How wide the dock is.
pub const DOCK_WIDTH: Pixels = px(380.);
/// How tall the dock is.
pub const DOCK_HEIGHT: Pixels = px(72.);
const COVER: Pixels = px(48.);
/// How much of the dock's height the wave along its bottom reaches up.
const WAVE: f32 = 0.7;
/// How strongly the wave shows behind the title and the buttons.
const WAVE_OPACITY: f32 = 0.55;

/// The little bar at the top of the desktop while TK-Music is in the background: the cover, the
/// track, the song controls and a wave that moves with the music.
pub struct DesktopDock {
    playback: Entity<Playback>,
    visualizer: VisualizerDrive,
    _observer: Subscription,
}

impl DesktopDock {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let playback = Sonora::global(cx).playback.clone();
        let _observer = cx.observe(&playback, |_, _, cx| cx.notify());
        Self {
            playback,
            visualizer: VisualizerDrive::default(),
            _observer,
        }
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
        let presence = self.visualizer.presence();
        let title: SharedString = track
            .as_ref()
            .map(|track| track.name.clone().into())
            .unwrap_or_default();
        let artists: SharedString = track
            .as_ref()
            .map(|track| track.artists.clone().into())
            .unwrap_or_default();
        let cover = track.as_ref().and_then(|track| track.cover.clone());
        let tint = theme
            .tint
            .map_or(theme.primary, |tint| theme.primary_of(tint));

        div()
            .id("desktop-dock")
            .role(Role::Group)
            .aria_label(t!("desktop-dock"))
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(theme.radius * 2.)
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .text_color(theme.foreground)
            .text_size(theme.text(Text::Body))
            .when(presence > 0., |this| {
                this.child(
                    Visualizer::new(self.visualizer.levels(), DOCK_HEIGHT * WAVE)
                        .style_kind(VisualizerStyle::Wave)
                        .tint(tint)
                        .behind(theme.background)
                        .opacity(presence * WAVE_OPACITY)
                        .corner_radius(theme.radius * 2.)
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0(),
                )
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap_3()
                    .size_full()
                    .px_3()
                    .child(Artwork::new(cover).size(COVER).corner_radius(theme.radius))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(theme.text(Text::Small))
                                    .text_color(theme.muted_foreground)
                                    .child(artists),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(previous(&self.playback, false, cx))
                            .child(toggle(&self.playback, false, false, cx))
                            .child(next(&self.playback, false, cx)),
                    ),
            )
    }
}
