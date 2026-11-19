import ActivityKit
import SwiftUI
import WidgetKit

@main
struct LiveLyricsBundle: WidgetBundle {
    var body: some Widget {
        LiveLyricsWidget()
    }
}

/// La frase que suena en la isla dinámica y en la pantalla bloqueada.
struct LiveLyricsWidget: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: LiveLyricsAttributes.self) { context in
            LockScreenView(state: context.state)
                .activityBackgroundTint(Color.black.opacity(0.78))
                .activitySystemActionForegroundColor(.white)
        } dynamicIsland: { context in
            let state = context.state
            let accent = Color(red: state.red, green: state.green, blue: state.blue)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Image(systemName: "music.note").font(.title3).foregroundColor(accent)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Image(systemName: state.playing ? "waveform" : "pause.fill").foregroundColor(accent)
                }
                DynamicIslandExpandedRegion(.center) {
                    Text(state.title).font(.caption.weight(.semibold)).lineLimit(1)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    VStack(spacing: 4) {
                        Text(state.line.isEmpty ? state.artist : state.line)
                            .font(.headline)
                            .multilineTextAlignment(.center)
                            .lineLimit(2)
                        if !state.next.isEmpty {
                            Text(state.next).font(.caption).foregroundColor(.white.opacity(0.5)).lineLimit(1)
                        }
                    }
                }
            } compactLeading: {
                Image(systemName: "music.note").foregroundColor(accent)
            } compactTrailing: {
                Text(state.line.isEmpty ? state.title : state.line)
                    .font(.caption2.weight(.semibold))
                    .lineLimit(1)
                    .frame(maxWidth: 84)
            } minimal: {
                Image(systemName: "music.note").foregroundColor(accent)
            }
        }
    }
}

private struct LockScreenView: View {
    let state: LiveLyricsAttributes.ContentState

    var body: some View {
        let accent = Color(red: state.red, green: state.green, blue: state.blue)
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Image(systemName: "music.note").foregroundColor(accent)
                Text(state.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                Text("· \(state.artist)").font(.subheadline).foregroundColor(.white.opacity(0.6)).lineLimit(1)
                Spacer(minLength: 0)
                Image(systemName: state.playing ? "waveform" : "pause.fill").foregroundColor(accent)
            }
            if !state.line.isEmpty {
                Text(state.line).font(.title3.weight(.bold)).lineLimit(2)
                if !state.next.isEmpty {
                    Text(state.next).font(.footnote).foregroundColor(.white.opacity(0.5)).lineLimit(1)
                }
            }
        }
        .foregroundColor(.white)
        .padding(16)
    }
}
