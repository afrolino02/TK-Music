import ActivityKit
import SwiftUI
import UIKit

/// Lleva la frase que suena a la Actividad en vivo (isla dinámica y pantalla bloqueada). Es lo
/// más parecido al dock de Android que permite el iPhone: Apple no deja dibujar encima de otras
/// apps. Va apagada hasta que se activa en Ajustes. Solo puede empezar con TK-Music abierta;
/// después se actualiza sola mientras suena la música.
@MainActor
final class LiveLyrics {
    static let shared = LiveLyrics()

    private var timer: Timer?
    private var key = ""
    private var lyrics: LyricsResult?
    private var accent: (Double, Double, Double) = (1, 0.48, 0.27)
    /// Lo último que se mandó (como `AnyHashable`: el tipo no existe antes de iOS 16.1).
    private var sent: AnyHashable?

    /// Empieza a revisar cada medio segundo lo que suena.
    func start(_ store: Store) {
        guard timer == nil else { return }
        timer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self, weak store] _ in
            guard let self, let store else { return }
            Task { @MainActor in self.tick(store) }
        }
    }

    private func tick(_ store: Store) {
        guard #available(iOS 16.2, *) else { return }
        let state = store.state
        guard store.liveLyrics, ActivityAuthorizationInfo().areActivitiesEnabled, let track = state.track, let slug = state.provider else {
            end()
            return
        }
        let wanted = "\(slug):\(track.id)"
        if wanted != key {
            key = wanted
            lyrics = nil
            Task { @MainActor in
                if case .success(let found) = await store.run({ try $0.lyricsFor(slug: slug, track: track) }), self.key == wanted {
                    self.lyrics = found
                }
                if let url = track.cover, let color = await ImagePipeline.shared.image(url)?.vividColors.first {
                    var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
                    UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
                    self.accent = (Double(r), Double(g), Double(b))
                }
            }
        }
        var line = ""
        var next = ""
        if let result = lyrics, result.synced {
            let position = store.smoothPosition(at: Date())
            if let active = result.lines.lastIndex(where: { $0.startMs <= position }) {
                line = result.lines[active].text
                if result.lines.indices.contains(active + 1) { next = result.lines[active + 1].text }
            } else if let first = result.lines.first {
                next = first.text
            }
        }
        let content = LiveLyricsAttributes.ContentState(
            title: track.title,
            artist: track.artists,
            line: line,
            next: next,
            playing: state.status == .playing,
            red: accent.0,
            green: accent.1,
            blue: accent.2
        )
        guard AnyHashable(content) != sent else { return }
        if let activity = Activity<LiveLyricsAttributes>.activities.first {
            sent = AnyHashable(content)
            Task { await activity.update(ActivityContent(state: content, staleDate: nil)) }
        } else if UIApplication.shared.applicationState == .active {
            // solo se puede empezar con la app abierta; si no, se intenta al volver
            if (try? Activity.request(attributes: LiveLyricsAttributes(), content: ActivityContent(state: content, staleDate: nil))) != nil {
                sent = AnyHashable(content)
            }
        }
    }

    private func end() {
        guard #available(iOS 16.2, *) else { return }
        sent = nil
        for activity in Activity<LiveLyricsAttributes>.activities {
            Task { await activity.end(nil, dismissalPolicy: .immediate) }
        }
    }
}
