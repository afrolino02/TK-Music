import SwiftUI

/// Los visuales que se mueven con la música en la pantalla completa.
enum VisualStyle: String, CaseIterable, Identifiable {
    case ondas, barras, anillo, particulas, aurora, vinilo, pulso, borde, electricidad, laseres
    var id: String { rawValue }
    var label: String {
        switch self {
        case .ondas: return "Ondas"
        case .barras: return "Barras espejo"
        case .anillo: return "Anillo"
        case .particulas: return "Partículas"
        case .aurora: return "Aurora"
        case .vinilo: return "Vinilo"
        case .pulso: return "Pulso"
        case .borde: return "Borde"
        case .electricidad: return "Electricidad"
        case .laseres: return "Láseres"
        }
    }
}

/// De dónde salen los colores de los visuales.
enum VisualColor: String, CaseIterable, Identifiable {
    case portada, arcoiris
    var id: String { rawValue }
    var label: String {
        switch self {
        case .portada: return "De la portada"
        case .arcoiris: return "Arcoíris"
        }
    }
}

/// La música como números para dibujar: las bandas suavizadas y la energía de graves, medios y
/// agudos, con un aviso en cada golpe del bajo.
final class Spectrum {
    static let count = 48
    private(set) var bands = [Float](repeating: 0, count: Spectrum.count)
    private(set) var bass: Float = 0
    private(set) var mid: Float = 0
    private(set) var high: Float = 0
    private(set) var level: Float = 0
    /// De 1 justo en el golpe a 0, apagándose.
    private(set) var kick: Float = 0
    /// Si en la última lectura hubo un golpe del bajo.
    private(set) var hit = false
    private var bassAverage: Float = 0.2
    private var lastHit = Date.distantPast

    func feed(_ raw: [Float]) {
        if raw.count >= 2 {
            let last = Float(raw.count - 1)
            for i in 0..<Spectrum.count {
                let at = Float(i) / Float(Spectrum.count - 1) * last
                let low = min(Int(at), raw.count - 1)
                let up = min(low + 1, raw.count - 1)
                let mix = at - Float(low)
                let target = min(max(raw[low] * (1 - mix) + raw[up] * mix, 0), 1)
                bands[i] += (target - bands[i]) * (target > bands[i] ? 0.5 : 0.15)
            }
        }
        bass = average(0, 6)
        mid = average(6, 24)
        high = average(24, Spectrum.count)
        level = average(0, Spectrum.count)
        bassAverage += (bass - bassAverage) * 0.04
        let now = Date()
        hit = bass > bassAverage * 1.22 && bass > 0.22 && now.timeIntervalSince(lastHit) > 0.22
        if hit {
            lastHit = now
            kick = 1
        } else {
            kick *= 0.88
        }
    }

    /// La banda en `x`, de 0 (graves) a 1 (agudos).
    func at(_ x: Double) -> Double {
        let p = min(max(x, 0), 1) * Double(Spectrum.count - 1)
        let i = min(Int(p), Spectrum.count - 2)
        return Double(bands[i]) + Double(bands[i + 1] - bands[i]) * (p - Double(i))
    }

    private func average(_ from: Int, _ to: Int) -> Float {
        bands[from..<to].reduce(0, +) / Float(to - from)
    }

    /// Lee la música unas 30 veces por segundo mientras alguien la mira.
    func listen(_ engine: Engine) async {
        while !Task.isCancelled {
            let raw = await Task.detached { engine.spectrum() }.value
            feed(raw)
            try? await Task.sleep(nanoseconds: 33_000_000)
        }
        engine.spectrumStop()
    }
}

/// Lo que algunos visuales recuerdan entre cuadros (partículas, rayos, el giro del disco).
final class VisualMemory {
    struct Particle {
        var angle: Double
        let base: Double
        let react: Double
        let size: Double
        let mix: Double
        let way: Double
        var radius: Double
        var speed: Double = 0
    }
    struct Bolt {
        let from: CGPoint
        let to: CGPoint
        var life: Double
        let span: Double
        let big: Bool
        let second: Bool
    }
    var particles: [Particle] = (0..<180).map { _ in
        let base = 0.18 + Double.random(in: 0..<0.6)
        return Particle(
            angle: Double.random(in: 0..<(2 * .pi)), base: base, react: 0.4 + Double.random(in: 0..<1),
            size: 0.6 + Double.random(in: 0..<1.6), mix: Double.random(in: 0..<1),
            way: Bool.random() ? 1 : -1, radius: base
        )
    }
    var bolts: [Bolt] = []
    var spin = 0.0
    var phase = 0.0
    var pulse = 0.0
    var last: Date?
}

/// Los dos colores del visual: los de la portada, o un arcoíris que gira despacio.
func visualColors(_ choice: VisualColor, glow: Color, depth: Color, seconds: Double) -> (Color, Color) {
    switch choice {
    case .portada:
        return (glow, depth)
    case .arcoiris:
        let hue = (seconds * 18).truncatingRemainder(dividingBy: 360) / 360
        return (Color(hue: hue, saturation: 0.75, brightness: 1), Color(hue: (hue + 70.0 / 360).truncatingRemainder(dividingBy: 1), saturation: 0.75, brightness: 1))
    }
}

private func mix(_ a: Color, _ b: Color, _ t: Double) -> Color {
    let first = UIColor(a), second = UIColor(b)
    var r1: CGFloat = 0, g1: CGFloat = 0, b1: CGFloat = 0, a1: CGFloat = 0
    var r2: CGFloat = 0, g2: CGFloat = 0, b2: CGFloat = 0, a2: CGFloat = 0
    first.getRed(&r1, green: &g1, blue: &b1, alpha: &a1)
    second.getRed(&r2, green: &g2, blue: &b2, alpha: &a2)
    let t = CGFloat(min(max(t, 0), 1))
    return Color(red: r1 + (r2 - r1) * t, green: g1 + (g2 - g1) * t, blue: b1 + (b2 - b1) * t)
}

/// Cuánto agranda Pulso la portada con cada golpe.
func pulseScale(_ style: VisualStyle, _ spectrum: Spectrum, calm: Bool) -> Double {
    style == .pulso ? 1 + 0.06 * Double(spectrum.kick) * (calm ? 0.3 : 1) : 1
}

/// El visual elegido en Ajustes. `focus` es el centro de la portada y `cover` su tamaño: los
/// visuales que la rodean (Anillo, Vinilo, Pulso, Electricidad) se dibujan alrededor. Ondas va
/// aparte (`StageWave`).
struct StageVisual: View {
    @EnvironmentObject private var store: Store
    let style: VisualStyle
    let glow: Color
    let depth: Color
    let spectrum: Spectrum
    let focus: CGPoint
    let cover: CGFloat
    @State private var memory = VisualMemory()
    @State private var start = Date()

    var body: some View {
        TimelineView(.animation) { timeline in
            Canvas { context, size in
                let now = timeline.date
                let dt = min(0.1, max(0, now.timeIntervalSince(memory.last ?? now.addingTimeInterval(-0.033))))
                memory.last = now
                let seconds = now.timeIntervalSince(start)
                let calm = store.visualCalm
                let (a, b) = visualColors(store.visualColor, glow: glow, depth: depth, seconds: seconds)
                var draw = Draw(
                    s: spectrum, a: a, b: b, time: seconds * (calm ? 0.35 : 1), dt: dt, center: focus,
                    radius: cover / 2, kickScale: calm ? 0.3 : 1, calm: calm, memory: memory
                )
                draw.paint(style, in: &context, size: size)
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Una franja chica para la barra de abajo: barras que suben desde el borde.
struct BarVisual: View {
    let spectrum: Spectrum
    let glow: Color
    let depth: Color
    @EnvironmentObject private var store: Store
    @State private var start = Date()

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1.0 / 30)) { timeline in
            Canvas { context, size in
                let (a, b) = visualColors(store.visualColor, glow: glow, depth: depth, seconds: timeline.date.timeIntervalSince(start))
                let count = max(24, Int(size.width / 9))
                let step = size.width / CGFloat(count)
                for i in 0..<count {
                    let v = spectrum.at(Double(i) / Double(count) * 0.8)
                    let h = 2 + v * size.height * 0.9
                    let rect = CGRect(x: CGFloat(i) * step + step * 0.2, y: size.height - h, width: step * 0.6, height: h)
                    context.fill(Path(roundedRect: rect, cornerRadius: step * 0.3), with: .color(mix(a, b, Double(i) / Double(count)).opacity(0.35)))
                }
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// Todo lo que un cuadro necesita para dibujarse.
private struct Draw {
    let s: Spectrum
    let a: Color
    let b: Color
    let time: Double
    let dt: Double
    let center: CGPoint
    let radius: CGFloat
    let kickScale: Double
    let calm: Bool
    let memory: VisualMemory

    mutating func paint(_ style: VisualStyle, in context: inout GraphicsContext, size: CGSize) {
        switch style {
        case .ondas: break
        case .barras: bars(&context, size)
        case .anillo: ring(&context, size)
        case .particulas: particles(&context, size)
        case .aurora: aurora(&context, size)
        case .vinilo: vinyl(&context, size)
        case .pulso: pulse(&context, size)
        case .borde: border(&context, size)
        case .electricidad: electricity(&context, size)
        case .laseres: lasers(&context, size)
        }
    }

    private func glowCircle(_ context: inout GraphicsContext, _ color: Color, _ at: CGPoint, _ r: CGFloat) {
        context.fill(
            Path(ellipseIn: CGRect(x: at.x - r, y: at.y - r, width: r * 2, height: r * 2)),
            with: .radialGradient(Gradient(colors: [color, color.opacity(0)]), center: at, startRadius: 0, endRadius: r)
        )
    }

    private func line(_ context: inout GraphicsContext, _ from: CGPoint, _ to: CGPoint, _ color: Color, _ width: CGFloat) {
        var path = Path()
        path.move(to: from)
        path.addLine(to: to)
        context.stroke(path, with: .color(color), style: StrokeStyle(lineWidth: width, lineCap: .round))
    }

    // Barras espejo: desde el centro hacia los lados, abajo, con su reflejo.
    func bars(_ context: inout GraphicsContext, _ size: CGSize) {
        let half = min(32, max(10, Int(size.width / 30)))
        let base = size.height * 0.86
        let step = size.width * 0.47 / CGFloat(half)
        let width = step * 0.58
        for k in 0..<half {
            let v = min(1, s.at(Double(k) / Double(half - 1) * 0.85))
            let h = 3 + CGFloat(v) * size.height * 0.2
            let color = mix(a, b, Double(k) / Double(half - 1))
            for side: CGFloat in [-1, 1] {
                let x = size.width / 2 + side * (CGFloat(k) + 0.5) * step - width / 2
                context.fill(Path(roundedRect: CGRect(x: x, y: base - h, width: width, height: h), cornerRadius: width / 2), with: .color(color.opacity(0.95)))
                context.fill(Path(roundedRect: CGRect(x: x, y: base + 4, width: width, height: h * 0.45), cornerRadius: width / 2), with: .color(color.opacity(0.18)))
            }
        }
    }

    // Anillo: rayos alrededor de la portada que crecen con la música, con un eco detrás.
    func ring(_ context: inout GraphicsContext, _ size: CGSize) {
        let r = radius * 1.15
        glowCircle(&context, a.opacity(0.18 + 0.3 * Double(s.level)), center, r * 2.6)
        let rays = 110
        let width = max(1.5, (2 * .pi * r / CGFloat(rays)) * 0.55)
        for j in 0..<rays {
            let f = Double(j) / Double(rays)
            let m = f < 0.5 ? f * 2 : (1 - f) * 2
            let v = min(1, s.at(m * 0.8))
            let angle = f * 2 * .pi - .pi / 2 + time * 0.04
            let dir = CGPoint(x: CGFloat(cos(angle)), y: CGFloat(sin(angle)))
            let r0 = r + 8
            let color = mix(b, a, m)
            func point(_ d: CGFloat) -> CGPoint { CGPoint(x: center.x + dir.x * d, y: center.y + dir.y * d) }
            let reach = CGFloat(v) * r
            line(&context, point(r0), point(r0 + 6 + reach * 1.25), color.opacity(0.22), width)
            line(&context, point(r0), point(r0 + 4 + reach * 0.85), color.opacity(0.95), width)
        }
    }

    // Partículas: puntos de luz en órbita que saltan hacia afuera con cada golpe.
    func particles(_ context: inout GraphicsContext, _ size: CGSize) {
        let r = min(size.width, size.height) * 0.5
        glowCircle(&context, a.opacity(0.08 + 0.18 * Double(s.level)), center, r * 0.6)
        context.blendMode = .plusLighter
        for index in memory.particles.indices {
            var p = memory.particles[index]
            p.angle += dt * (0.06 + 0.3 * Double(s.mid)) * p.way * (calm ? 0.35 : 1)
            if s.hit { p.speed += 1.3 * p.react * kickScale }
            p.speed += (p.base - p.radius) * 9 * dt
            p.speed *= 0.9
            p.radius += p.speed * dt
            memory.particles[index] = p
            let at = CGPoint(x: center.x + CGFloat(cos(p.angle) * p.radius) * r * 1.4, y: center.y + CGFloat(sin(p.angle) * p.radius) * r)
            let d = CGFloat(p.size * (1 + Double(s.high) * 2.2) * 1.6)
            context.fill(Path(ellipseIn: CGRect(x: at.x - d, y: at.y - d, width: d * 2, height: d * 2)), with: .color(mix(a, b, p.mix).opacity(0.85)))
        }
        context.blendMode = .normal
    }

    // Aurora: cortinas de luz que flotan y se ondulan con los medios.
    func aurora(_ context: inout GraphicsContext, _ size: CGSize) {
        context.blendMode = .plusLighter
        for i in 0..<4 {
            let color = i % 2 == 1 ? b : a
            let base = size.height * (0.18 + 0.1 * CGFloat(i))
            let amp = size.height * (0.05 + 0.1 * CGFloat(s.mid))
            var top = Path()
            var curtain = Path()
            var points: [CGPoint] = []
            var x: CGFloat = 0
            while x <= size.width + 16 {
                let xd = Double(x)
                let swing = sin(xd * 0.004 + time * 0.35 * (1 + Double(i) * 0.3) + Double(i)) + sin(xd * 0.011 - time * 0.6 + Double(i) * 2) * 0.4
                let music = s.at(xd / Double(size.width) * 0.7)
                let y = base + CGFloat(swing) * amp - CGFloat(music) * size.height * 0.08
                points.append(CGPoint(x: x, y: y))
                x += 16
            }
            top.addLines(points)
            curtain.addLines(points)
            for p in points.reversed() { curtain.addLine(to: CGPoint(x: p.x, y: p.y + size.height * 0.4)) }
            curtain.closeSubpath()
            context.fill(curtain, with: .linearGradient(
                Gradient(stops: [
                    .init(color: color.opacity(0), location: 0),
                    .init(color: color.opacity(0.18 + 0.35 * Double(s.level)), location: 0.15),
                    .init(color: color.opacity(0), location: 1),
                ]),
                startPoint: CGPoint(x: 0, y: base - amp * 2), endPoint: CGPoint(x: 0, y: base + size.height * 0.4)
            ))
            context.stroke(top, with: .color(color.opacity(0.15 + 0.3 * Double(s.level))), lineWidth: 1.5)
        }
        context.blendMode = .normal
    }

    // Vinilo: un disco que gira saliendo de detrás de la portada; los surcos brillan con la música.
    func vinyl(_ context: inout GraphicsContext, _ size: CGSize) {
        memory.spin += dt * 0.9 * (calm ? 0.35 : 1)
        let r = radius * 0.95 * CGFloat(1 + Double(s.kick) * 0.012 * kickScale)
        let disc = CGPoint(x: center.x + radius * 0.62, y: center.y)
        glowCircle(&context, a.opacity(0.2 + 0.4 * Double(s.level)), disc, r * 1.7)
        let ring = { (d: CGFloat) in Path(ellipseIn: CGRect(x: disc.x - d, y: disc.y - d, width: d * 2, height: d * 2)) }
        context.fill(ring(r), with: .color(Color(white: 0.04)))
        var g = r * 0.42
        while g < r * 0.97 {
            let v = s.at(Double((g / r - 0.42) / 0.55) * 0.8)
            context.stroke(ring(g), with: .color(.white.opacity(0.025 + 0.09 * v)), lineWidth: 1)
            g += max(2.2, r / 70)
        }
        for start in [-18.0, 162.0] {
            var wedge = Path()
            wedge.move(to: disc)
            wedge.addArc(center: disc, radius: r * 0.97, startAngle: .degrees(start + memory.spin * 57.3), endAngle: .degrees(start + 36 + memory.spin * 57.3), clockwise: false)
            wedge.closeSubpath()
            context.fill(wedge, with: .color(.white.opacity(0.06)))
        }
        context.fill(ring(r * 0.34), with: .color(a))
        context.fill(ring(r * 0.12), with: .color(b))
        context.fill(ring(r * 0.03 + 2), with: .color(Color(white: 0.04)))
    }

    // Pulso: luces suaves de los colores de la portada detrás, que crecen con el volumen.
    func pulse(_ context: inout GraphicsContext, _ size: CGSize) {
        let base = radius * 2
        context.blendMode = .plusLighter
        for (k, color) in [a, b, a].enumerated() {
            let phase = Double(k) * 1.6
            let at = CGPoint(
                x: center.x + CGFloat(cos(time * 0.4 + phase)) * base * 0.35 + CGFloat(k - 1) * base * 0.15,
                y: center.y + CGFloat(sin(time * 0.33 + phase)) * base * 0.22
            )
            glowCircle(&context, color.opacity(0.32), at, base * (0.75 + CGFloat(s.level) * 0.9))
        }
        context.blendMode = .normal
    }

    // Borde: una línea fina pegada al borde con dos zonas brillantes que corren al ritmo.
    func border(_ context: inout GraphicsContext, _ size: CGSize) {
        memory.pulse = max(memory.pulse * 0.92, Double(s.kick) * kickScale)
        memory.phase = (memory.phase + dt * (0.05 + 0.3 * Double(s.level)) * (calm ? 0.35 : 1)).truncatingRemainder(dividingBy: 1)
        let bright = 0.5 + 0.5 * min(1, memory.pulse)
        let sigma = 0.09 + 0.05 * Double(s.level)
        func glow(_ u: Double) -> Double {
            func near(_ d: Double) -> Double {
                let w = (d.truncatingRemainder(dividingBy: 1) + 1.5).truncatingRemainder(dividingBy: 1) - 0.5
                return exp(-(w * w) / (2 * sigma * sigma))
            }
            return bright * (0.12 + 0.88 * max(near(u - memory.phase), near(u - memory.phase - 0.5)))
        }
        func shading(_ alpha: Double, whiten: Double) -> GraphicsContext.Shading {
            let stops: [Gradient.Stop] = (0...24).map { k in
                let u = Double(k) / 24
                let g = glow(u)
                let color = mix(mix(a, b, 0.5 + 0.5 * sin((u - memory.phase) * 2 * .pi)), .white, whiten * g)
                return .init(color: color.opacity(min(1, alpha * g)), location: u)
            }
            return .conicGradient(Gradient(stops: stops), center: CGPoint(x: size.width / 2, y: size.height / 2), angle: .degrees(-90))
        }
        let width: CGFloat = 3
        let frame = Path(roundedRect: CGRect(x: width / 2, y: width / 2, width: size.width - width, height: size.height - width), cornerRadius: min(size.width, size.height) * 0.035)
        context.blendMode = .plusLighter
        for k in stride(from: 14, through: 1, by: -1) {
            context.stroke(frame, with: shading(0.075 * (1 - Double(k) / 15) + 0.01, whiten: 0), lineWidth: width * (1 + CGFloat(k) * 1.7))
        }
        context.blendMode = .normal
        context.stroke(frame, with: shading(1.15, whiten: 0.35), lineWidth: width)
    }

    // Electricidad: rayos que salen de la portada con cada golpe del bajo, y chispas alrededor.
    func electricity(_ context: inout GraphicsContext, _ size: CGSize) {
        let r0 = radius * 1.05
        func spawn(_ big: Bool) {
            let angle = Double.random(in: 0..<(2 * .pi))
            let far = max(size.width, size.height) * (big ? 0.75 : 0.22 + CGFloat.random(in: 0..<0.2))
            let bend = angle + Double.random(in: -0.25..<0.25)
            memory.bolts.append(.init(
                from: CGPoint(x: center.x + CGFloat(cos(angle)) * r0, y: center.y + CGFloat(sin(angle)) * r0),
                to: CGPoint(x: center.x + CGFloat(cos(bend)) * far, y: center.y + CGFloat(sin(bend)) * far),
                life: big ? 0.32 : 0.14, span: big ? 0.32 : 0.14, big: big, second: Bool.random()
            ))
        }
        if s.hit { for _ in 0..<(2 + Int(3 * kickScale)) { spawn(true) } }
        if Double.random(in: 0..<1) < Double(s.high) * dt * 14 { spawn(false) }
        memory.pulse = s.hit ? 1 : memory.pulse * 0.85
        if memory.pulse > 0.02 {
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(mix(a, .white, 0.5).opacity(0.08 * memory.pulse * kickScale)))
        }
        memory.bolts = memory.bolts.compactMap { bolt in
            var bolt = bolt
            bolt.life -= dt
            return bolt.life > 0 ? bolt : nil
        }
        context.blendMode = .plusLighter
        for bolt in memory.bolts {
            let fade = bolt.life / bolt.span * Double.random(in: 0.6..<1.0)
            let color = bolt.second ? b : a
            let length = hypot(bolt.to.x - bolt.from.x, bolt.to.y - bolt.from.y)
            var paths = [jagged(bolt.from, bolt.to, length * 0.28, 6)]
            if bolt.big, paths[0].count > 12 {
                for _ in 0..<2 {
                    let from = paths[0][4 + Int.random(in: 0..<(paths[0].count - 12))]
                    let angle = Double(atan2(bolt.to.y - bolt.from.y, bolt.to.x - bolt.from.x)) + Double.random(in: -0.8..<0.8)
                    let branch = length * (0.2 + CGFloat.random(in: 0..<0.25))
                    paths.append(jagged(from, CGPoint(x: from.x + CGFloat(cos(angle)) * branch, y: from.y + CGFloat(sin(angle)) * branch), branch * 0.3, 4))
                }
            }
            for (i, points) in paths.enumerated() {
                var path = Path()
                path.addLines(points)
                let width: CGFloat = (bolt.big ? 1 : 0.6) * (i > 0 ? 0.6 : 1)
                context.stroke(path, with: .color(color.opacity(0.12 * fade)), style: StrokeStyle(lineWidth: 9 * width, lineCap: .round, lineJoin: .round))
                context.stroke(path, with: .color(color.opacity(0.4 * fade)), style: StrokeStyle(lineWidth: 3.5 * width, lineCap: .round, lineJoin: .round))
                context.stroke(path, with: .color(mix(color, .white, 0.75).opacity(0.95 * fade)), style: StrokeStyle(lineWidth: 1.4 * width, lineCap: .round, lineJoin: .round))
            }
        }
        context.blendMode = .normal
        for _ in 0..<Int(6 + Double(s.level) * 30) {
            let angle = Double.random(in: 0..<(2 * .pi))
            let r = r0 * (1.02 + CGFloat.random(in: 0..<0.06))
            let bend = angle + Double.random(in: -0.075..<0.075)
            let r2 = r + CGFloat.random(in: 0..<(r0 * 0.14))
            let start = CGPoint(x: center.x + CGFloat(cos(angle)) * r, y: center.y + CGFloat(sin(angle)) * r)
            let end = CGPoint(x: center.x + CGFloat(cos(bend)) * r2, y: center.y + CGFloat(sin(bend)) * r2)
            line(&context, start, end, mix(b, .white, 0.5).opacity(0.5), 1)
        }
    }

    // Láseres: rayos desde abajo y desde las esquinas de arriba que barren la pantalla al ritmo.
    func lasers(_ context: inout GraphicsContext, _ size: CGSize) {
        let far = hypot(size.width, size.height) * 1.2
        let scale = max(0.6, min(size.width, size.height) / 500)
        let sources: [(CGPoint, Double, Double)] = [
            (CGPoint(x: size.width * 0.18, y: size.height + 4), -.pi / 2, 0.05),
            (CGPoint(x: size.width * 0.39, y: size.height + 4), -.pi / 2, 0.2),
            (CGPoint(x: size.width * 0.61, y: size.height + 4), -.pi / 2, 0.38),
            (CGPoint(x: size.width * 0.82, y: size.height + 4), -.pi / 2, 0.55),
            (CGPoint(x: -4, y: -4), .pi / 4, 0.7),
            (CGPoint(x: size.width + 4, y: -4), .pi * 3 / 4, 0.8),
        ]
        context.blendMode = .plusLighter
        for (k, source) in sources.enumerated() {
            let (at, base, band) = source
            let v = min(1, s.at(band))
            let bright = min(1.2, max(0, 0.2 + v * 0.9 + Double(s.kick) * 0.4 * kickScale))
            let sweep = sin(time * (0.7 + Double(k) * 0.13) + Double(k) * 1.7) * (k < 4 ? 0.55 : 0.35)
            let spread = 0.06 + 0.16 * Double(s.mid)
            let color = k % 2 == 1 ? b : a
            for j in -1...1 {
                let angle = base + sweep + Double(j) * spread
                let end = CGPoint(x: at.x + CGFloat(cos(angle)) * far, y: at.y + CGFloat(sin(angle)) * far)
                line(&context, at, end, color.opacity(min(1, 0.05 * bright)), 14 * scale)
                line(&context, at, end, color.opacity(min(1, 0.2 * bright)), 4 * scale)
                line(&context, at, end, mix(color, .white, 0.6).opacity(min(1, 0.9 * bright)), 1.3 * scale)
            }
            glowCircle(&context, mix(color, .white, 0.5).opacity(0.8 * min(1, bright)), at, 30 * scale)
        }
        context.blendMode = .normal
    }

    /// Un rayo quebrado: cada mitad se tuerce un poco, varias veces.
    private func jagged(_ from: CGPoint, _ to: CGPoint, _ bend: CGFloat, _ depth: Int) -> [CGPoint] {
        var out = [from]
        func split(_ p: CGPoint, _ q: CGPoint, _ d: CGFloat, _ left: Int) {
            guard left > 0 else {
                out.append(q)
                return
            }
            let mid = CGPoint(x: (p.x + q.x) / 2 + CGFloat.random(in: -0.5..<0.5) * d, y: (p.y + q.y) / 2 + CGFloat.random(in: -0.5..<0.5) * d)
            split(p, mid, d / 2, left - 1)
            split(mid, q, d / 2, left - 1)
        }
        split(from, to, bend, depth)
        return out
    }
}
