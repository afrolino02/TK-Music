import SwiftUI

/// La letra de lo que suena, dentro del reproductor.
struct LyricsPanel: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        let state = store.state
        if let track = state.track, let slug = state.provider {
            Loader(key: "\(slug):\(track.id)", load: { try $0.lyricsFor(slug: slug, track: track) }) { result in
                LyricsBody(result: result)
            }
            // mientras se ven las letras, la pantalla no se apaga (karaoke, proyector)
            .onAppear { UIApplication.shared.isIdleTimerDisabled = true }
            .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
        } else {
            Text("No suena nada.").foregroundStyle(.secondary)
        }
    }
}

/// El botón para mostrar u ocultar la pronunciación, solo si la letra la tiene.
struct RomanizedToggle: View {
    @EnvironmentObject private var store: Store
    let result: LyricsResult

    var body: some View {
        if result.lines.contains(where: { $0.romanized != nil }) {
            Toggle("Pronunciación", isOn: $store.showRomanized)
                .toggleStyle(.button)
                .font(.footnote.weight(.semibold))
                .buttonStyle(.bordered)
        }
    }
}

struct LyricsBody: View {
    @EnvironmentObject private var store: Store
    let result: LyricsResult?

    var body: some View {
        if let result, !result.lines.isEmpty {
            if result.synced {
                SyncedLyrics(result: result)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        RomanizedToggle(result: result)
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(Array(result.lines.enumerated()), id: \.offset) { _, line in
                                Text(line.text.isEmpty ? " " : line.text)
                                    .font(.system(size: 20, weight: .semibold))
                                if store.showRomanized, let reading = line.romanized {
                                    Text(reading)
                                        .font(.system(size: 15, weight: .regular))
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                        Text("Letra: \(result.source)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 24)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .scrollIndicators(.hidden)
            }
        } else if result?.instrumental == true {
            Label("Instrumental", systemImage: "music.note")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            Text("No encontré la letra de esta canción.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

/// Como en el escritorio: la línea que suena, nítida y grande, a un tercio de la altura; las
/// demás, más pequeñas, tenues y desenfocadas. Tocar una línea salta a ese momento.
struct SyncedLyrics: View {
    @EnvironmentObject private var store: Store
    let result: LyricsResult

    var body: some View {
        let worded = result.lines.contains { !$0.words.isEmpty }
        TimelineView(.periodic(from: .now, by: worded ? 0.05 : 0.25)) { context in
            sheet(position: store.smoothPosition(at: context.date))
        }
    }

    private func sheet(position: UInt64) -> some View {
        let active = activeLine(at: position)
        return GeometryReader { geometry in
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        Color.clear.frame(height: geometry.size.height * 0.3)
                        lines(active: active, position: position)
                        Text("Letra: \(result.source)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .padding(.top, 16)
                        Color.clear.frame(height: geometry.size.height * 0.6)
                    }
                }
                .scrollIndicators(.hidden)
                .overlay(alignment: .topTrailing) { RomanizedToggle(result: result).padding(8) }
                .onChange(of: active) { line in
                    guard let line else { return }
                    withAnimation(.easeInOut(duration: 0.8)) {
                        proxy.scrollTo(line, anchor: UnitPoint(x: 0.5, y: 0.3))
                    }
                }
                .onAppear {
                    if let active { proxy.scrollTo(active, anchor: UnitPoint(x: 0.5, y: 0.3)) }
                }
            }
        }
    }

    private func lines(active: Int?, position: UInt64) -> some View {
        ForEach(Array(result.lines.enumerated()), id: \.offset) { index, line in
            let distance: Int = active.map { abs(index - $0) } ?? 99
            let current = index == active
            LyricRow(
                text: line.text,
                romanized: store.showRomanized ? line.romanized : nil,
                words: current ? line.words : [],
                position: current ? position : 0,
                current: current,
                distance: distance
            )
                .id(index)
                .onTapGesture { store.engine.seek(positionMs: line.startMs) }
        }
    }

    private func activeLine(at position: UInt64) -> Int? {
        result.lines.lastIndex(where: { $0.startMs <= position })
    }
}

/// Una línea de la letra. Todas ocupan el espacio de la línea grande y las demás se encogen con
/// una escala, así el texto no cambia de tamaño ni empuja a las otras al pasar de una a otra.
private struct LyricRow: View {
    let text: String
    let romanized: String?
    let words: [LyricWord]
    let position: UInt64
    let current: Bool
    let distance: Int

    /// Karaoke: en la línea que suena, las palabras ya cantadas se encienden y las demás esperan.
    private var styled: Text {
        guard current, !words.isEmpty else { return Text(text.isEmpty ? "♪" : text) }
        return words.reduce(Text("")) { line, word in
            line + Text(word.text).foregroundColor(word.startMs <= position ? .primary : .primary.opacity(0.4))
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            styled
                .font(.system(size: 28, weight: .bold))
            // la pronunciación, más pequeña, debajo de la línea en otro sistema de escritura
            if let romanized {
                Text(romanized)
                    .font(.system(size: 16, weight: .medium))
                    .opacity(0.75)
            }
        }
            .foregroundStyle(.primary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .scaleEffect(current ? 1 : 0.8, anchor: .leading)
            .opacity(current ? 1 : (distance <= 1 ? 0.5 : 0.3))
            .blur(radius: current ? 0 : (distance <= 1 ? 1.5 : 3))
            .animation(.easeInOut(duration: 0.7), value: current)
            .animation(.easeInOut(duration: 0.7), value: distance)
            .padding(.vertical, 6)
            .contentShape(Rectangle())
    }
}
