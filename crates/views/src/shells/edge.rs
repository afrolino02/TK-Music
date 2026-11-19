use gpui::prelude::*;
use gpui::{Context, Entity, Subscription, Window, div};
use state::{AppSettings, Playback, Sonora};
use ui::{ActiveTheme as _, Visualizer, VisualizerStyle};

use crate::shared::visualizer::VisualizerDrive;

/// The glow round the edge of the whole desktop: the border scene alone, in a transparent window
/// that stands over every app and lets clicks through. It shows while a track plays and sinks
/// away when it stops, so the window can stay open while the setting is on.
pub struct DesktopEdge {
    playback: Entity<Playback>,
    settings: Entity<AppSettings>,
    visualizer: VisualizerDrive,
    _observers: [Subscription; 2],
}

impl DesktopEdge {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let sonora = Sonora::global(cx);
        let (playback, settings) = (sonora.playback.clone(), sonora.settings.clone());
        let _observers = [
            cx.observe(&playback, |_, _, cx| cx.notify()),
            cx.observe(&settings, |_, _, cx| cx.notify()),
        ];
        Self {
            playback,
            settings,
            visualizer: VisualizerDrive::default(),
            _observers,
        }
    }
}

impl Render for DesktopEdge {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let playback = self.playback.read(cx);
        let spectrum = playback
            .wants_playing()
            .then(|| playback.spectrum(cx))
            .flatten();
        match spectrum {
            Some(spectrum) => self.visualizer.show(cx.entity_id(), spectrum, window),
            None => self.visualizer.hide(),
        }
        let presence = self.visualizer.presence();
        let settings = self.settings.read(cx);
        let theme = *cx.theme();
        let tint = theme
            .tint
            .map_or(theme.primary, |tint| theme.primary_of(tint));

        div().size_full().when(presence > 0., |this| {
            this.child(
                Visualizer::new(self.visualizer.levels(), window.viewport_size().height)
                    .style_kind(VisualizerStyle::Border)
                    .tint(tint)
                    .rainbow(settings.visualizer_rainbow())
                    .calm(settings.visualizer_calm())
                    .opacity(presence)
                    .size_full(),
            )
        })
    }
}
