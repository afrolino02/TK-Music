import SwiftUI

/// Cómo se anima la letra sincronizada.
enum LyricStyle: String, CaseIterable, Identifiable {
    case enfoque, karaoke, cinematica, palabra, escribir, chat
    var id: String { rawValue }
    var label: String {
        switch self {
        case .enfoque: return "Enfoque"
        case .karaoke: return "Karaoke"
        case .cinematica: return "Cinemática"
        case .palabra: return "Palabra por palabra"
        case .escribir: return "Máquina de escribir"
        case .chat: return "Chat"
        }
    }
}

/// Qué va detrás de la letra.
enum LyricBackdrop: String, CaseIterable, Identifiable {
    case portada, color, visual
    var id: String { rawValue }
    var label: String {
        switch self {
        case .portada: return "Portada desenfocada"
        case .color: return "Color de la portada"
        case .visual: return "Con el visual"
        }
    }
}

/// Los dos colores de la portada que suena; los pone el reproductor.
struct LyricAccent {
    var glow: Color
    var depth: Color
}

private struct LyricAccentKey: EnvironmentKey {
    static let defaultValue = LyricAccent(glow: .white, depth: .white)
}

extension EnvironmentValues {
    var lyricAccent: LyricAccent {
        get { self[LyricAccentKey.self] }
        set { self[LyricAccentKey.self] = newValue }
    }
}

/// El azul de las burbujas propias del Chat, el amarillo de su palabra que suena y la tinta del Papel.
private let sentBlue = Color(red: 0.24, green: 0.48, blue: 0.95)
private let sentWord = Color(red: 1, green: 0.886, blue: 0.478)
private let paperInk = Color(red: 0.12, green: 0.165, blue: 0.36)

/// Las palabras de una línea con sus tiempos. Casi ningún servicio los trae palabra por palabra:
/// entonces se reparten en la línea según el largo de cada palabra. Juntas forman el texto.
func timedWords(_ line: LyricLine, next: LyricLine?) -> [LyricWord] {
    if !line.words.isEmpty { return line.words }
    if line.text.trimmingCharacters(in: .whitespaces).isEmpty { return [] }
    let start = Double(line.startMs)
    let end = max(start + 400, Double(line.endMs ?? next?.startMs ?? (line.startMs + 4000)))
    let span = min((end - start) * 0.85, 7000)
    var parts: [String] = []
    var current = ""
    for character in line.text {
        if character == " ", !current.isEmpty, current.last != " " {
            current.append(character)
        } else if current.last == " " {
            parts.append(current)
            current = String(character)
        } else {
            current.append(character)
        }
    }
    if !current.isEmpty { parts.append(current) }
    let weights = parts.map { Double($0.trimmingCharacters(in: .whitespaces).count) + 1.5 }
    let total = weights.reduce(0, +)
    var at = start
    return parts.enumerated().map { index, part in
        let length = span * weights[index] / total
        defer { at += length }
        return LyricWord(startMs: UInt64(at), endMs: UInt64(at + length), text: part)
    }
}

/// Cuánto de una palabra ya se cantó, de 0 a 1.
private func progress(_ word: LyricWord, _ position: UInt64) -> Double {
    let length = max(1, Double(word.endMs) - Double(word.startMs))
    return min(1, max(0, (Double(position) - Double(word.startMs)) / length))
}

/// La frase pintada hasta donde va la voz: lo cantado en `lit`, lo que falta en `dim`.
private func painted(_ words: [LyricWord], _ position: UInt64, lit: Color, dim: Color) -> Text {
    words.reduce(Text("")) { text, word in
        let trimmed = word.text.trimmingCharacters(in: .whitespaces).count
        let n = Int((Double(trimmed) * progress(word, position)).rounded())
        let done = String(word.text.prefix(n))
        let rest = String(word.text.dropFirst(n))
        return text + Text(done).foregroundColor(lit) + Text(rest).foregroundColor(dim)
    }
}

/// Las líneas que se repiten en la canción: casi siempre el coro.
private func chorus(_ result: LyricsResult) -> [Bool] {
    var counts: [String: Int] = [:]
    for line in result.lines { counts[line.text.trimmingCharacters(in: .whitespaces).lowercased(), default: 0] += 1 }
    return result.lines.map {
        let key = $0.text.trimmingCharacters(in: .whitespaces).lowercased()
        return !key.isEmpty && (counts[key] ?? 0) >= 2
    }
}

private func activeLine(_ result: LyricsResult, _ position: UInt64) -> Int? {
    result.lines.lastIndex(where: { $0.startMs <= position })
}

/// El color para resaltar, legible sobre el fondo del reproductor.
@MainActor
private struct Highlight {
    let store: Store
    let accent: LyricAccent
    let light: Bool
    var color: Color { store.lyricWhite ? (light ? .black : .white) : accent.glow }
}

/// La letra sincronizada con el estilo elegido en Ajustes. Enfoque es la lista de siempre.
struct StyledLyrics: View {
    @EnvironmentObject private var store: Store
    @Environment(\.lyricAccent) private var accent
    @Environment(\.colorScheme) private var scheme
    let result: LyricsResult

    var body: some View {
        if store.lyricStyle == .enfoque {
            SyncedLyrics(result: result)
        } else {
            TimelineView(.animation) { context in
                let position = store.smoothPosition(at: context.date)
                styled(position: position, date: context.date)
            }
        }
    }

    @ViewBuilder
    private func styled(position: UInt64, date: Date) -> some View {
        let active = activeLine(result, position)
        let scale = store.lyricSize
        let lit = Highlight(store: store, accent: accent, light: scheme == .light).color
        switch store.lyricStyle {
        case .karaoke: KaraokeView(result: result, position: position, active: active, scale: scale, lit: lit)
        case .cinematica: CinematicView(result: result, active: active, scale: scale)
        case .palabra: WordByWordView(result: result, position: position, active: active, scale: scale, lit: lit)
        case .escribir: TypewriterView(result: result, position: position, active: active, scale: scale, lit: lit, date: date)
        case .chat: ChatView(result: result, position: position, active: active, scale: scale, lit: lit)
        case .enfoque: EmptyView()
        }
    }
}

/// El menú de la pantalla de letras para cambiar de estilo sin ir a Ajustes.
struct LyricStyleMenu: View {
    @EnvironmentObject private var store: Store

    var body: some View {
        Menu {
            Picker("Estilo de la letra", selection: $store.lyricStyle) {
                ForEach(LyricStyle.allCases) { Text($0.label).tag($0) }
            }
        } label: {
            Label(store.lyricStyle.label, systemImage: "textformat")
                .font(.footnote.weight(.semibold))
        }
        .buttonStyle(.bordered)
    }
}

/// Antes de la primera línea: tres puntos que se encienden mientras llega la voz.
private struct Waiting: View {
    let result: LyricsResult
    let position: UInt64
    let color: Color

    var body: some View {
        let first = max(1, Double(result.lines.first?.startMs ?? 1))
        let lit = Int(Double(position) / first * 3.999)
        HStack(spacing: 10) {
            ForEach(0..<3, id: \.self) { k in
                Circle().fill(color.opacity(k <= lit ? 1 : 0.25)).frame(width: 10, height: 10)
            }
        }
    }
}

/// Centrado a lo alto, alineado según Ajustes.
private struct OneLine<Content: View>: View {
    @EnvironmentObject private var store: Store
    @ViewBuilder let content: Content

    var body: some View {
        content
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: store.lyricStart ? .leading : .center)
            .multilineTextAlignment(store.lyricStart ? .leading : .center)
            .padding(.horizontal, 8)
    }
}

private extension AnyTransition {
    /// Entra desde abajo y sale hacia arriba, con un fundido.
    static var lineSwap: AnyTransition {
        .asymmetric(
            insertion: .opacity.combined(with: .offset(y: 24)),
            removal: .opacity.combined(with: .offset(y: -24))
        )
    }
}

// MARK: Karaoke

private struct KaraokeView: View {
    @EnvironmentObject private var store: Store
    let result: LyricsResult
    let position: UInt64
    let active: Int?
    let scale: Double
    let lit: Color

    var body: some View {
        OneLine {
            if let active {
                let line = result.lines[active]
                let next = result.lines.indices.contains(active + 1) ? result.lines[active + 1] : nil
                VStack(alignment: store.lyricStart ? .leading : .center, spacing: 14) {
                    painted(timedWords(line, next: next), position, lit: lit, dim: .primary.opacity(0.4))
                        .font(.system(size: 30 * scale, weight: .heavy))
                    if let next {
                        Text(next.text).font(.system(size: 18 * scale, weight: .bold)).foregroundStyle(.primary.opacity(0.4))
                    }
                }
                .id(active)
                .transition(.lineSwap)
            } else {
                Waiting(result: result, position: position, color: lit)
            }
        }
        .animation(.easeOut(duration: 0.4), value: active)
    }
}

// MARK: Cinemática

/// Entra desenfocada desde abajo y sale hacia arriba.
private struct BlurLift: ViewModifier {
    let amount: Double
    let rise: Double
    func body(content: Content) -> some View {
        content.blur(radius: 14 * amount).opacity(1 - amount).offset(y: rise * amount)
    }
}

private struct CinematicView: View {
    let result: LyricsResult
    let active: Int?
    let scale: Double

    var body: some View {
        OneLine {
            ZStack {
                let text = active.map { result.lines[$0].text } ?? ""
                Text(text.isEmpty ? "♪" : text)
                    .font(.system(size: 34 * scale, weight: .medium, design: .serif).italic())
                    .id(active ?? -1)
                    .transition(.asymmetric(
                        insertion: .modifier(active: BlurLift(amount: 1, rise: 24), identity: BlurLift(amount: 0, rise: 24)),
                        removal: .modifier(active: BlurLift(amount: 1, rise: -24), identity: BlurLift(amount: 0, rise: -24))
                    ))
            }
        }
        .animation(.easeInOut(duration: 0.9), value: active)
    }
}

// MARK: Palabra por palabra

/// Acomoda las vistas en renglones, como un párrafo.
private struct FlowLayout: Layout {
    var spacing: CGFloat = 8
    var lineSpacing: CGFloat = 2
    var center = true

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        let width = rows.map(\.width).max() ?? 0
        let height = rows.map(\.height).reduce(0, +) + lineSpacing * CGFloat(max(0, rows.count - 1))
        return CGSize(width: min(width, proposal.width ?? width), height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in arrange(width: bounds.width, subviews: subviews) {
            var x = center ? bounds.minX + (bounds.width - row.width) / 2 : bounds.minX
            for index in row.items {
                let size = subviews[index].sizeThatFits(.unspecified)
                subviews[index].place(at: CGPoint(x: x, y: y + (row.height - size.height)), proposal: .unspecified)
                x += size.width + spacing
            }
            y += row.height + lineSpacing
        }
    }

    private struct Row { var items: [Int] = []; var width: CGFloat = 0; var height: CGFloat = 0 }

    private func arrange(width: CGFloat, subviews: Subviews) -> [Row] {
        var rows: [Row] = [Row()]
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let extra = rows[rows.count - 1].items.isEmpty ? size.width : rows[rows.count - 1].width + spacing + size.width
            if extra > width, !rows[rows.count - 1].items.isEmpty {
                rows.append(Row())
            }
            var row = rows[rows.count - 1]
            row.width = row.items.isEmpty ? size.width : row.width + spacing + size.width
            row.height = max(row.height, size.height)
            row.items.append(index)
            rows[rows.count - 1] = row
        }
        return rows
    }
}

private struct WordByWordView: View {
    @EnvironmentObject private var store: Store
    let result: LyricsResult
    let position: UInt64
    let active: Int?
    let scale: Double
    let lit: Color

    var body: some View {
        OneLine {
            if let active {
                let line = result.lines[active]
                let next = result.lines.indices.contains(active + 1) ? result.lines[active + 1] : nil
                VStack(alignment: store.lyricStart ? .leading : .center, spacing: 18) {
                    FlowLayout(spacing: 9 * scale, center: !store.lyricStart) {
                        ForEach(Array(timedWords(line, next: next).enumerated()), id: \.offset) { _, word in
                            let on = position >= word.startMs
                            let now = on && position < word.endMs
                            Text(word.text.trimmingCharacters(in: .whitespaces))
                                .font(.system(size: 32 * scale, weight: .heavy))
                                .foregroundStyle(now ? lit : .primary)
                                .scaleEffect(on ? 1 : 0.55)
                                .opacity(on ? 1 : 0)
                                .offset(y: on ? 0 : 16)
                                .animation(.spring(response: 0.35, dampingFraction: 0.45), value: on)
                        }
                    }
                    if let next {
                        Text(next.text).font(.system(size: 16 * scale, weight: .bold)).foregroundStyle(.primary.opacity(0.4))
                    }
                }
                .id(active)
                .transition(.opacity)
            } else {
                Waiting(result: result, position: position, color: lit)
            }
        }
        .animation(.easeOut(duration: 0.2), value: active)
    }
}

// MARK: Máquina de escribir

private struct TypewriterView: View {
    @EnvironmentObject private var store: Store
    let result: LyricsResult
    let position: UInt64
    let active: Int?
    let scale: Double
    let lit: Color
    let date: Date

    var body: some View {
        OneLine {
            if let active {
                let line = result.lines[active]
                let next = result.lines.indices.contains(active + 1) ? result.lines[active + 1] : nil
                let typed = timedWords(line, next: next).reduce(into: (text: "", done: false)) { state, word in
                    guard !state.done else { return }
                    let n = Int((progress(word, position) * Double(word.text.count)).rounded(.up))
                    state.text += String(word.text.prefix(n))
                    if n < word.text.count { state.done = true }
                }.text
                let caret = Int(date.timeIntervalSinceReferenceDate * 2) % 2 == 0
                VStack(alignment: store.lyricStart ? .leading : .center, spacing: 16) {
                    if active > 0 {
                        Text(result.lines[active - 1].text)
                            .font(.system(size: 15 * scale, design: .monospaced))
                            .foregroundStyle(.primary.opacity(0.4))
                    }
                    (Text(typed.trimmingCharacters(in: .whitespaces)) + Text("▌").foregroundColor(lit.opacity(caret ? 1 : 0)))
                        .font(.system(size: 24 * scale, weight: .medium, design: .monospaced))
                }
            } else {
                Waiting(result: result, position: position, color: lit)
            }
        }
    }
}

// MARK: Chat

/// Lugares conocidos que, si la frase los nombra, llegan con su tarjeta de mapa.
private let places: [(String, String)] = [
    ("cartagena", "Cartagena, Colombia"), ("barranquilla", "Barranquilla, Colombia"), ("medellín", "Medellín, Colombia"),
    ("medellin", "Medellín, Colombia"), ("bogotá", "Bogotá, Colombia"), ("bogota", "Bogotá, Colombia"), ("cali", "Cali, Colombia"),
    ("santa marta", "Santa Marta, Colombia"), ("san andrés", "San Andrés, Colombia"), ("puerto rico", "Puerto Rico"),
    ("san juan", "San Juan, Puerto Rico"), ("la habana", "La Habana, Cuba"), ("santo domingo", "Santo Domingo, R. D."),
    ("caracas", "Caracas, Venezuela"), ("panamá", "Panamá"), ("ciudad de méxico", "Ciudad de México"), ("méxico", "México"),
    ("buenos aires", "Buenos Aires, Argentina"), ("lima", "Lima, Perú"), ("santiago", "Santiago, Chile"), ("madrid", "Madrid, España"),
    ("barcelona", "Barcelona, España"), ("ibiza", "Ibiza, España"), ("parís", "París, Francia"), ("paris", "París, Francia"),
    ("londres", "Londres, Reino Unido"), ("london", "London, UK"), ("tokyo", "Tokio, Japón"), ("tokio", "Tokio, Japón"),
    ("new york", "New York, USA"), ("nueva york", "Nueva York, USA"), ("brooklyn", "Brooklyn, New York"), ("manhattan", "Manhattan, New York"),
    ("hollywood", "Hollywood, Los Angeles"), ("los angeles", "Los Angeles, USA"), ("miami", "Miami, USA"), ("las vegas", "Las Vegas, USA"),
    ("chicago", "Chicago, USA"), ("atlanta", "Atlanta, USA"), ("houston", "Houston, USA"), ("toronto", "Toronto, Canadá"),
    ("malecón", "El Malecón"), ("malecon", "El Malecón"),
]

private func place(in text: String) -> String? {
    let lower = " " + text.lowercased() + " "
    for (word, name) in places {
        guard let range = lower.range(of: word) else { continue }
        let before = lower[lower.index(before: range.lowerBound)]
        let after = lower[range.upperBound]
        if !before.isLetter, !after.isLetter { return name }
    }
    return nil
}

private struct ChatView: View {
    let result: LyricsResult
    let position: UInt64
    let active: Int?
    let scale: Double
    let lit: Color

    var body: some View {
        let sides = chorus(result)
        let shown = (active ?? -1) + 1
        let first = max(0, shown - 7)
        let next = result.lines.indices.contains(shown) ? result.lines[shown] : nil
        let typing = next.map { !sides[shown] && $0.startMs > position && $0.startMs - position <= 1100 } ?? false
        VStack(spacing: 6) {
            Spacer(minLength: 0)
            ForEach(first..<shown, id: \.self) { j in
                let line = result.lines[j]
                let next = result.lines.indices.contains(j + 1) ? result.lines[j + 1] : nil
                Bubble(
                    line: line,
                    words: timedWords(line, next: next),
                    out: sides[j],
                    age: shown - 1 - j,
                    position: j == active ? position : .max,
                    scale: scale,
                    lit: lit
                )
                .transition(.asymmetric(
                    insertion: .scale(scale: 0.5, anchor: sides[j] ? .bottomTrailing : .bottomLeading)
                        .combined(with: .opacity)
                        .combined(with: .offset(x: sides[j] ? 40 : -40)),
                    removal: .opacity
                ))
            }
            if typing {
                TypingBubble().transition(.opacity.combined(with: .scale(scale: 0.6, anchor: .bottomLeading)))
            }
        }
        .frame(maxWidth: 460)
        .padding(.horizontal, 12)
        .frame(maxWidth: .infinity)
        .frame(maxHeight: .infinity, alignment: .top)
        .padding(.bottom, 80)
        .animation(.spring(response: 0.45, dampingFraction: 0.62), value: shown)
        .animation(.easeOut(duration: 0.25), value: typing)
    }
}

private let fades: [Double] = [1, 0.9, 0.62, 0.36, 0.18, 0.08, 0]
private let blurs: [Double] = [0, 0, 1, 2, 3, 4, 5]

private struct Bubble: View {
    let line: LyricLine
    let words: [LyricWord]
    let out: Bool
    let age: Int
    let position: UInt64
    let scale: Double
    let lit: Color

    var body: some View {
        let step = min(age, fades.count - 1)
        let where_ = place(in: line.text)
        let now = out ? sentWord : lit
        let text = words.isEmpty ? Text(line.text.isEmpty ? "♪" : line.text) : words.reduce(Text("")) { text, word in
            let color: Color = position >= word.startMs && position < word.endMs ? now
                : position >= word.startMs ? .white : .white.opacity(0.42)
            return text + Text(word.text).foregroundColor(color)
        }
        HStack {
            if out { Spacer(minLength: 40) }
            VStack(alignment: .leading, spacing: 0) {
                if where_ != nil { MapCard(pin: lit) }
                text
                    .font(.system(size: 16 * scale, weight: where_ != nil ? .semibold : .medium))
                    .padding(.horizontal, 13)
                    .padding(.vertical, 8)
                if let where_ {
                    Text("📍 \(where_)")
                        .font(.system(size: 11 * scale))
                        .foregroundStyle(.white.opacity(0.75))
                        .padding([.horizontal, .bottom], 13)
                        .padding(.top, -4)
                }
            }
            .background(out ? sentBlue : Color(red: 0.165, green: 0.157, blue: 0.188))
            .clipShape(UnevenRoundedCorners(out: out))
            .frame(maxWidth: 320, alignment: out ? .trailing : .leading)
            if !out { Spacer(minLength: 40) }
        }
        .opacity(fades[step])
        .blur(radius: blurs[step])
        .scaleEffect(1 - Double(step) * 0.025, anchor: out ? .bottomTrailing : .bottomLeading)
        .animation(.easeOut(duration: 0.45), value: step)
    }
}

/// Esquinas redondeadas salvo la de abajo del lado de quien habla (iOS 16 no trae la forma).
private struct UnevenRoundedCorners: Shape {
    let out: Bool
    func path(in rect: CGRect) -> Path {
        let big: CGFloat = 18, small: CGFloat = 5
        let bl = out ? big : small, br = out ? small : big
        var path = Path()
        path.move(to: CGPoint(x: rect.minX + big, y: rect.minY))
        path.addLine(to: CGPoint(x: rect.maxX - big, y: rect.minY))
        path.addArc(center: CGPoint(x: rect.maxX - big, y: rect.minY + big), radius: big, startAngle: .degrees(-90), endAngle: .degrees(0), clockwise: false)
        path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY - br))
        path.addArc(center: CGPoint(x: rect.maxX - br, y: rect.maxY - br), radius: br, startAngle: .degrees(0), endAngle: .degrees(90), clockwise: false)
        path.addLine(to: CGPoint(x: rect.minX + bl, y: rect.maxY))
        path.addArc(center: CGPoint(x: rect.minX + bl, y: rect.maxY - bl), radius: bl, startAngle: .degrees(90), endAngle: .degrees(180), clockwise: false)
        path.addLine(to: CGPoint(x: rect.minX, y: rect.minY + big))
        path.addArc(center: CGPoint(x: rect.minX + big, y: rect.minY + big), radius: big, startAngle: .degrees(180), endAngle: .degrees(270), clockwise: false)
        path.closeSubpath()
        return path
    }
}

/// Un mapa dibujado (calles, agua y un alfiler), no uno de verdad.
private struct MapCard: View {
    let pin: Color
    var body: some View {
        Canvas { context, size in
            let w = size.width, h = size.height
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(Color(red: 0.106, green: 0.149, blue: 0.22)))
            var water = Path()
            water.move(to: .zero); water.addLine(to: CGPoint(x: w * 0.28, y: 0))
            water.addCurve(to: CGPoint(x: w * 0.2, y: h), control1: CGPoint(x: w * 0.22, y: h * 0.3), control2: CGPoint(x: w * 0.32, y: h * 0.6))
            water.addLine(to: CGPoint(x: 0, y: h)); water.closeSubpath()
            context.fill(water, with: .color(Color(red: 0.086, green: 0.208, blue: 0.337)))
            let road = Color(red: 0.188, green: 0.255, blue: 0.36)
            for (a, b) in [((0.3, 0.15), (1.0, 0.42)), ((0.42, 0.0), (0.54, 1.0)), ((0.67, 0.0), (0.75, 1.0)), ((0.29, 0.72), (1.0, 0.66))] {
                var p = Path(); p.move(to: CGPoint(x: w * a.0, y: h * a.1)); p.addLine(to: CGPoint(x: w * b.0, y: h * b.1))
                context.stroke(p, with: .color(road), lineWidth: 5)
            }
            let center = CGPoint(x: w * 0.6, y: h * 0.5)
            context.fill(Path(ellipseIn: CGRect(x: center.x - 9, y: center.y - 9, width: 18, height: 18)), with: .color(pin))
            context.fill(Path(ellipseIn: CGRect(x: center.x - 3.5, y: center.y - 3.5, width: 7, height: 7)), with: .color(.white))
        }
        .frame(minWidth: 220, maxWidth: .infinity)
        .frame(height: 64)
    }
}

private struct TypingBubble: View {
    var body: some View {
        HStack {
            TimelineView(.animation) { context in
                let t = context.date.timeIntervalSinceReferenceDate
                HStack(spacing: 5) {
                    ForEach(0..<3, id: \.self) { k in
                        let lift = (sin(t * 6 - Double(k) * 0.9) + 1) / 2
                        Circle().fill(.white.opacity(0.5 + 0.5 * lift)).frame(width: 7, height: 7).offset(y: -4 * lift)
                    }
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .background(Color(red: 0.165, green: 0.157, blue: 0.188))
                .clipShape(UnevenRoundedCorners(out: false))
            }
            Spacer(minLength: 40)
        }
    }
}

// MARK: - La línea que suena, fuera de la pantalla de letras

/// La letra de lo que suena, para la pantalla completa y la barra de abajo.
@MainActor
final class CurrentLyrics: ObservableObject {
    @Published var result: LyricsResult?
    private var key = ""

    func load(_ store: Store) async {
        guard let track = store.state.track, let slug = store.state.provider else { return }
        let wanted = "\(slug):\(track.id)"
        guard wanted != key else { return }
        key = wanted
        result = nil
        if case .success(let found) = await store.run({ try $0.lyricsFor(slug: slug, track: track) }) {
            if key == wanted { result = found }
        }
    }
}

/// La frase que suena, grande, encima del visual de la pantalla completa.
struct StageLine: View {
    @EnvironmentObject private var store: Store
    @Environment(\.lyricAccent) private var accent
    @StateObject private var lyrics = CurrentLyrics()
    let big: Bool

    var body: some View {
        Group {
            if let result = lyrics.result, result.synced, !result.lines.isEmpty {
                TimelineView(.animation) { context in
                    let position = store.smoothPosition(at: context.date)
                    let active = activeLine(result, position)
                    ZStack {
                        if let active, !result.lines[active].text.isEmpty {
                            let next = result.lines.indices.contains(active + 1) ? result.lines[active + 1] : nil
                            painted(timedWords(result.lines[active], next: next), position, lit: store.lyricWhite ? .white : accent.glow, dim: .primary.opacity(0.55))
                                .font(.system(size: 26 * store.lyricSize * (big ? 1.2 : 1), weight: .heavy))
                                .multilineTextAlignment(.center)
                                .lineLimit(2)
                                .id(active)
                                .transition(.lineSwap)
                        }
                    }
                    .animation(.easeOut(duration: 0.4), value: active)
                }
            }
        }
        .task(id: store.state.track?.id) { await lyrics.load(store) }
    }
}

/// La segunda línea de la barra de abajo: la frase que suena, o el artista.
struct BarLine: View {
    @EnvironmentObject private var store: Store
    @StateObject private var lyrics = CurrentLyrics()
    let fallback: String

    var body: some View {
        Group {
            if store.lyricsInBar, store.state.status == .playing, let result = lyrics.result, result.synced, !result.lines.isEmpty {
                TimelineView(.periodic(from: .now, by: 0.1)) { context in
                    let position = store.smoothPosition(at: context.date)
                    if let active = activeLine(result, position), !result.lines[active].text.isEmpty {
                        let next = result.lines.indices.contains(active + 1) ? result.lines[active + 1] : nil
                        painted(timedWords(result.lines[active], next: next), position, lit: .primary, dim: .secondary)
                    } else {
                        Text(fallback).foregroundStyle(.secondary)
                    }
                }
            } else {
                Text(fallback).foregroundStyle(.secondary)
            }
        }
        .font(.caption)
        .lineLimit(1)
        .task(id: store.lyricsInBar ? store.state.track?.id : nil) {
            if store.lyricsInBar { await lyrics.load(store) }
        }
    }
}
