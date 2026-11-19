package org.sonora.mobile

import android.os.SystemClock
import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin
import kotlin.random.Random

/** Los visuales que se mueven con la música en la pantalla completa. */
enum class VisualStyle(val label: String) {
    ONDAS("Ondas"),
    BARRAS("Barras espejo"),
    ANILLO("Anillo"),
    PARTICULAS("Partículas"),
    AURORA("Aurora"),
    VINILO("Vinilo"),
    PULSO("Pulso"),
    BORDE("Borde"),
    ELECTRICIDAD("Electricidad"),
    LASERES("Láseres"),
}

/** De dónde salen los colores de los visuales. */
enum class VisualColor(val label: String) {
    PORTADA("De la portada"),
    ARCOIRIS("Arcoíris"),
}

/** Cuántas bandas usan los visuales, de graves a agudos. */
private const val BANDS = 48

/**
 * La música como números para dibujar: las bandas suavizadas y la energía de graves, medios y
 * agudos, con un aviso (`hit`) en cada golpe del bajo. Cambia `tick` en cada lectura, para que
 * los dibujos se vuelvan a pintar.
 */
class Spectrum {
    val bands = FloatArray(BANDS)
    var bass = 0f
        private set
    var mid = 0f
        private set
    var high = 0f
        private set
    var level = 0f
        private set
    /** De 1 justo en el golpe a 0, apagándose. */
    var kick = 0f
        private set
    /** Si en esta lectura hubo un golpe del bajo. */
    var hit = false
        private set
    var tick by mutableLongStateOf(0L)
        private set
    private var bassAverage = 0.2f
    private var lastHit = 0L

    fun feed(raw: FloatArray, now: Long) {
        if (raw.size >= 2) {
            for (i in 0 until BANDS) {
                val at = i.toFloat() / (BANDS - 1) * raw.lastIndex
                val low = at.toInt().coerceIn(0, raw.lastIndex)
                val high = (low + 1).coerceAtMost(raw.lastIndex)
                val target = (raw[low] * (1 - (at - low)) + raw[high] * (at - low)).coerceIn(0f, 1f)
                bands[i] += (target - bands[i]) * if (target > bands[i]) 0.5f else 0.15f
            }
        }
        bass = average(0, 6)
        mid = average(6, 24)
        high = average(24, BANDS)
        level = average(0, BANDS)
        bassAverage += (bass - bassAverage) * 0.04f
        hit = bass > bassAverage * 1.22f && bass > 0.22f && now - lastHit > 220
        if (hit) {
            lastHit = now
            kick = 1f
        } else {
            kick *= 0.88f
        }
        tick = now
    }

    /** La banda en `x`, de 0 (graves) a 1 (agudos). */
    fun at(x: Float): Float {
        val p = x.coerceIn(0f, 1f) * (BANDS - 1)
        val i = p.toInt().coerceAtMost(BANDS - 2)
        return bands[i] + (bands[i + 1] - bands[i]) * (p - i)
    }

    private fun average(from: Int, to: Int): Float {
        var sum = 0f
        for (i in from until to) sum += bands[i]
        return sum / (to - from)
    }
}

/** Lee la música unas 30 veces por segundo mientras el dibujo está a la vista. */
@Composable
fun rememberSpectrum(): Spectrum {
    val spectrum = remember { Spectrum() }
    LaunchedEffect(Unit) {
        try {
            while (true) {
                val raw = withContext(Dispatchers.Default) { runCatching { Store.engine.spectrum() }.getOrDefault(emptyList()) }
                spectrum.feed(raw.toFloatArray(), SystemClock.uptimeMillis())
                delay(33)
            }
        } finally {
            runCatching { Store.engine.spectrumStop() }
        }
    }
    return spectrum
}

/** Los dos colores del visual: los de la portada, o un arcoíris que gira despacio. */
fun visualColors(glow: Color, depth: Color, seconds: Float): Pair<Color, Color> = when (Store.visualColor) {
    VisualColor.PORTADA -> glow to depth
    VisualColor.ARCOIRIS -> {
        val hue = (seconds * 18f) % 360f
        Color.hsv(hue, 0.75f, 1f) to Color.hsv((hue + 70f) % 360f, 0.75f, 1f)
    }
}

/** Lo que algunos visuales recuerdan entre cuadros (partículas, rayos, el giro del disco). */
private class Memory {
    val particles = List(180) {
        Particle(
            angle = Random.nextFloat() * 2f * PI.toFloat(),
            base = 0.18f + Random.nextFloat() * 0.6f,
            react = 0.4f + Random.nextFloat(),
            size = 0.6f + Random.nextFloat() * 1.6f,
            mix = Random.nextFloat(),
            way = if (Random.nextBoolean()) 1f else -1f,
        )
    }
    val bolts = mutableListOf<Bolt>()
    var spin = 0f
    var phase = 0f
    var pulse = 0f
    var last = 0L
}

private class Particle(var angle: Float, val base: Float, val react: Float, val size: Float, val mix: Float, val way: Float) {
    var radius = base
    var speed = 0f
}

private class Bolt(val from: Offset, val to: Offset, var life: Float, val span: Float, val big: Boolean, val second: Boolean)

/**
 * El visual elegido en Ajustes, a pantalla completa. `focus` es el centro de la portada y
 * `cover` su tamaño: los visuales que la rodean (Anillo, Vinilo, Pulso, Electricidad) se
 * dibujan alrededor. Ondas va aparte ([StageWave]).
 */
@Composable
fun StageVisual(
    style: VisualStyle,
    glow: Color,
    depth: Color,
    spectrum: Spectrum,
    focus: Pair<Dp, Dp>,
    cover: Dp,
    modifier: Modifier = Modifier,
    strength: Float = 1f,
) {
    val memory = remember { Memory() }
    val start = remember { SystemClock.uptimeMillis() }
    Canvas(modifier) {
        spectrum.tick // repinta con cada lectura
        val now = SystemClock.uptimeMillis()
        val seconds = (now - start) / 1000f
        val dt = if (memory.last == 0L) 0.033f else ((now - memory.last) / 1000f).coerceIn(0f, 0.1f)
        memory.last = now
        val calm = Store.visualCalm
        val time = seconds * if (calm) 0.35f else 1f
        val kickScale = if (calm) 0.3f else 1f
        val (a, b) = visualColors(glow, depth, seconds)
        val center = Offset(focus.first.toPx(), focus.second.toPx())
        val radius = cover.toPx() * 0.5f
        val draw = Draw(spectrum, a, b, time, dt, center, radius, kickScale, strength, memory)
        when (style) {
            VisualStyle.ONDAS -> Unit
            VisualStyle.BARRAS -> draw.bars(this)
            VisualStyle.ANILLO -> draw.ring(this)
            VisualStyle.PARTICULAS -> draw.particles(this)
            VisualStyle.AURORA -> draw.aurora(this)
            VisualStyle.VINILO -> draw.vinyl(this)
            VisualStyle.PULSO -> draw.pulse(this)
            VisualStyle.BORDE -> draw.border(this)
            VisualStyle.ELECTRICIDAD -> draw.electricity(this)
            VisualStyle.LASERES -> draw.lasers(this)
        }
    }
}

/** Cuánto agranda Pulso la portada con cada golpe, para que el reproductor la escale. */
fun pulseScale(style: VisualStyle, spectrum: Spectrum): Float {
    spectrum.tick
    if (style != VisualStyle.PULSO) return 1f
    return 1f + 0.06f * spectrum.kick * if (Store.visualCalm) 0.3f else 1f
}

/** Una franja chica para la barra de abajo: barras que suben desde el borde. */
@Composable
fun BarVisual(glow: Color, depth: Color, spectrum: Spectrum, modifier: Modifier = Modifier) {
    val start = remember { SystemClock.uptimeMillis() }
    Canvas(modifier) {
        spectrum.tick
        val (a, b) = visualColors(glow, depth, (SystemClock.uptimeMillis() - start) / 1000f)
        val count = max(24, (size.width / 9.dp.toPx()).toInt())
        val step = size.width / count
        for (i in 0 until count) {
            val v = spectrum.at(i.toFloat() / count * 0.8f)
            val h = 2.dp.toPx() + v * size.height * 0.9f
            drawRoundRect(
                lerp(a, b, i.toFloat() / count).copy(alpha = 0.35f),
                topLeft = Offset(i * step + step * 0.2f, size.height - h),
                size = Size(step * 0.6f, h),
                cornerRadius = CornerRadius(step * 0.3f),
            )
        }
    }
}

/** Todo lo que un cuadro necesita para dibujarse. */
private class Draw(
    val s: Spectrum,
    val a: Color,
    val b: Color,
    val time: Float,
    val dt: Float,
    val center: Offset,
    val radius: Float,
    val kickScale: Float,
    val strength: Float,
    val memory: Memory,
) {
    // Barras espejo: desde el centro hacia los lados, abajo, con su reflejo.
    fun bars(scope: DrawScope) = with(scope) {
        val half = (size.width / 30.dp.toPx()).toInt().coerceIn(10, 32)
        val base = size.height * 0.86f
        val step = size.width * 0.47f / half
        val width = step * 0.58f
        for (k in 0 until half) {
            val v = min(1f, s.at(k.toFloat() / (half - 1) * 0.85f) * strength)
            val h = 3.dp.toPx() + v * size.height * 0.2f
            val color = lerp(a, b, k.toFloat() / (half - 1))
            for (side in intArrayOf(-1, 1)) {
                val x = size.width / 2f + side * (k + 0.5f) * step - width / 2f
                drawRoundRect(color.copy(alpha = 0.95f), Offset(x, base - h), Size(width, h), CornerRadius(width / 2f))
                drawRoundRect(color.copy(alpha = 0.18f), Offset(x, base + 4.dp.toPx()), Size(width, h * 0.45f), CornerRadius(width / 2f))
            }
        }
    }

    // Anillo: rayos alrededor de la portada que crecen con la música, con un eco detrás.
    fun ring(scope: DrawScope) = with(scope) {
        val r = radius * 1.15f
        drawCircle(
            Brush.radialGradient(listOf(a.copy(alpha = 0.18f + 0.3f * s.level), Color.Transparent), center, r * 2.6f),
            r * 2.6f,
            center,
        )
        val rays = 110
        val width = max(1.5f, (2f * PI.toFloat() * r / rays) * 0.55f)
        val spin = time * 0.04f
        for (j in 0 until rays) {
            val f = j.toFloat() / rays
            val m = if (f < 0.5f) f * 2 else (1 - f) * 2
            val v = min(1f, s.at(m * 0.8f) * strength)
            val angle = f * 2f * PI.toFloat() - PI.toFloat() / 2 + spin
            val r0 = r + 8.dp.toPx()
            val color = lerp(b, a, m)
            val dir = Offset(cos(angle), sin(angle))
            drawLine(color.copy(alpha = 0.22f), center + dir * r0, center + dir * (r0 + 6 + v * r * 1.25f), width, StrokeCap.Round)
            drawLine(color.copy(alpha = 0.95f), center + dir * r0, center + dir * (r0 + 4 + v * r * 0.85f), width, StrokeCap.Round)
        }
    }

    // Partículas: puntos de luz en órbita que saltan hacia afuera con cada golpe.
    fun particles(scope: DrawScope) = with(scope) {
        val r = min(size.width, size.height) * 0.5f
        drawCircle(Brush.radialGradient(listOf(a.copy(alpha = 0.08f + 0.18f * s.level), Color.Transparent), center, r * 0.6f), r * 0.6f, center)
        for (p in memory.particles) {
            p.angle += dt * (0.06f + 0.3f * s.mid * strength) * p.way * (if (kickScale < 1f) 0.35f else 1f)
            if (s.hit) p.speed += 1.3f * p.react * strength * kickScale
            p.speed += (p.base - p.radius) * 9f * dt
            p.speed *= 0.9f
            p.radius += p.speed * dt
            val at = center + Offset(cos(p.angle) * p.radius * r * 1.4f, sin(p.angle) * p.radius * r)
            drawCircle(lerp(a, b, p.mix).copy(alpha = 0.85f), p.size * (1 + s.high * 2.2f) * 1.6.dp.toPx(), at, blendMode = BlendMode.Plus)
        }
    }

    // Aurora: cortinas de luz que flotan y se ondulan con los medios.
    fun aurora(scope: DrawScope) = with(scope) {
        for (i in 0 until 4) {
            val color = if (i % 2 == 1) b else a
            val base = size.height * (0.18f + 0.1f * i)
            val amp = size.height * (0.05f + 0.1f * s.mid * strength)
            val top = Path()
            val bottom = mutableListOf<Offset>()
            var x = 0f
            while (x <= size.width + 16f) {
                val y = base + sin(x * 0.004f + time * 0.35f * (1 + i * 0.3f) + i) * amp +
                    sin(x * 0.011f - time * 0.6f + i * 2) * amp * 0.4f - s.at(x / size.width * 0.7f) * size.height * 0.08f
                if (x == 0f) top.moveTo(x, y) else top.lineTo(x, y)
                bottom += Offset(x, y)
                x += 16f
            }
            val curtain = Path().apply {
                addPath(top)
                for (p in bottom.asReversed()) lineTo(p.x, p.y + size.height * 0.4f)
                close()
            }
            drawPath(
                curtain,
                Brush.verticalGradient(
                    0f to color.copy(alpha = 0f),
                    0.15f to color.copy(alpha = 0.18f + 0.35f * s.level),
                    1f to color.copy(alpha = 0f),
                    startY = base - amp * 2,
                    endY = base + size.height * 0.4f,
                ),
                blendMode = BlendMode.Plus,
            )
            drawPath(top, lerp(color, Color.White, 0.3f).copy(alpha = 0.15f + 0.3f * s.level), style = Stroke(1.5.dp.toPx()))
        }
    }

    // Vinilo: un disco que gira saliendo de detrás de la portada; los surcos brillan con la música.
    fun vinyl(scope: DrawScope) = with(scope) {
        memory.spin += dt * 0.9f * (if (kickScale < 1f) 0.35f else 1f)
        val r = radius * 0.95f * (1 + s.kick * 0.012f * kickScale)
        val disc = center + Offset(radius * 0.62f, 0f)
        drawCircle(Brush.radialGradient(listOf(a.copy(alpha = 0.2f + 0.4f * s.level), Color.Transparent), disc, r * 1.7f), r * 1.7f, disc)
        drawCircle(Color(0xFF0A0A0C), r, disc)
        var g = r * 0.42f
        while (g < r * 0.97f) {
            val v = s.at((g / r - 0.42f) / 0.55f * 0.8f)
            drawCircle(Color.White.copy(alpha = 0.025f + 0.09f * v * strength), g, disc, style = Stroke(1f))
            g += max(2.2f, r / 70f)
        }
        rotate(memory.spin * 57.3f, disc) {
            drawArc(Color.White.copy(alpha = 0.06f), -18f, 36f, true, disc - Offset(r * 0.97f, r * 0.97f), Size(r * 1.94f, r * 1.94f))
            drawArc(Color.White.copy(alpha = 0.06f), 162f, 36f, true, disc - Offset(r * 0.97f, r * 0.97f), Size(r * 1.94f, r * 1.94f))
        }
        drawCircle(a, r * 0.34f, disc)
        drawCircle(b, r * 0.12f, disc)
        drawCircle(Color(0xFF0A0A0C), r * 0.03f + 2f, disc)
    }

    // Pulso: luces suaves de los colores de la portada detrás, que crecen con el volumen.
    fun pulse(scope: DrawScope) = with(scope) {
        val base = radius * 2f
        for ((k, color) in listOf(a, b, a).withIndex()) {
            val phase = k * 1.6f
            val at = center + Offset(cos(time * 0.4f + phase) * base * 0.35f + (k - 1) * base * 0.15f, sin(time * 0.33f + phase) * base * 0.22f)
            val r = base * (0.75f + s.level * 0.9f * strength)
            drawCircle(Brush.radialGradient(listOf(color.copy(alpha = 0.32f), Color.Transparent), at, r), r, at, blendMode = BlendMode.Plus)
        }
    }

    // Borde: una línea fina pegada al borde con dos zonas brillantes que corren al ritmo.
    fun border(scope: DrawScope) = with(scope) {
        memory.pulse = max(memory.pulse * 0.92f, s.kick * kickScale)
        memory.phase = (memory.phase + dt * (0.05f + 0.3f * s.level * strength) * (if (kickScale < 1f) 0.35f else 1f)) % 1f
        val bright = 0.5f + 0.5f * min(1f, memory.pulse * strength)
        val sigma = 0.09f + 0.05f * s.level
        fun glowAt(u: Float): Float {
            fun near(d: Float): Float {
                val w = ((d % 1f) + 1.5f) % 1f - 0.5f
                return exp(-(w * w) / (2 * sigma * sigma))
            }
            return bright * (0.12f + 0.88f * max(near(u - memory.phase), near(u - memory.phase - 0.5f)))
        }
        fun sweep(alpha: Float, whiten: Float): Brush {
            val stops = Array(25) { k ->
                val u = k / 24f
                val g = glowAt(u)
                val color = lerp(a, b, 0.5f + 0.5f * sin((u - memory.phase) * 2f * PI.toFloat()))
                u to lerp(color, Color.White, whiten * g).copy(alpha = min(1f, alpha * g))
            }
            return Brush.sweepGradient(*stops, center = Offset(size.width / 2f, size.height / 2f))
        }
        val line = 3.dp.toPx()
        val corner = CornerRadius(min(size.width, size.height) * 0.035f)
        for (k in 14 downTo 1) {
            val w = line * (1 + k * 1.7f)
            drawRoundRect(sweep(0.075f * (1 - k / 15f) + 0.01f, 0f), Offset(line / 2, line / 2), Size(size.width - line, size.height - line), corner, style = Stroke(w), blendMode = BlendMode.Plus)
        }
        drawRoundRect(sweep(1.15f, 0.35f), Offset(line / 2, line / 2), Size(size.width - line, size.height - line), corner, style = Stroke(line))
    }

    // Electricidad: rayos que salen de la portada con cada golpe del bajo, y chispas alrededor.
    fun electricity(scope: DrawScope) = with(scope) {
        val r0 = radius * 1.05f
        fun spawn(big: Boolean) {
            val angle = Random.nextFloat() * 2f * PI.toFloat()
            val far = max(size.width, size.height) * if (big) 0.75f else 0.22f + Random.nextFloat() * 0.2f
            val bend = angle + (Random.nextFloat() - 0.5f) * 0.5f
            memory.bolts += Bolt(
                center + Offset(cos(angle), sin(angle)) * r0,
                center + Offset(cos(bend), sin(bend)) * far,
                if (big) 0.32f else 0.14f, if (big) 0.32f else 0.14f, big, Random.nextBoolean(),
            )
        }
        if (s.hit) repeat(2 + (3 * min(1.5f, strength) * kickScale).toInt()) { spawn(true) }
        if (Random.nextFloat() < s.high * dt * 14f * strength) spawn(false)
        memory.pulse = if (s.hit) 1f else memory.pulse * 0.85f
        if (memory.pulse > 0.02f) drawRect(lerp(a, Color.White, 0.5f).copy(alpha = 0.08f * memory.pulse * kickScale))
        memory.bolts.removeAll { bolt -> bolt.life -= dt; bolt.life <= 0f }
        for (bolt in memory.bolts) {
            val fade = bolt.life / bolt.span * (0.6f + Random.nextFloat() * 0.4f)
            val color = if (bolt.second) b else a
            val main = jagged(bolt.from, bolt.to, (bolt.to - bolt.from).getDistance() * 0.28f, 6)
            val paths = mutableListOf(main)
            if (bolt.big && main.size > 12) repeat(2) {
                val from = main[4 + Random.nextInt(main.size - 12)]
                val angle = kotlin.math.atan2(bolt.to.y - bolt.from.y, bolt.to.x - bolt.from.x) + (Random.nextFloat() - 0.5f) * 1.6f
                val length = (bolt.to - bolt.from).getDistance() * (0.2f + Random.nextFloat() * 0.25f)
                paths += jagged(from, from + Offset(cos(angle), sin(angle)) * length, length * 0.3f, 4)
            }
            for ((i, points) in paths.withIndex()) {
                val path = Path().apply {
                    moveTo(points[0].x, points[0].y)
                    for (p in points) lineTo(p.x, p.y)
                }
                val width = (if (bolt.big) 1f else 0.6f) * (if (i > 0) 0.6f else 1f) * 1.dp.toPx()
                drawPath(path, color.copy(alpha = 0.12f * fade), style = Stroke(9 * width), blendMode = BlendMode.Plus)
                drawPath(path, color.copy(alpha = 0.4f * fade), style = Stroke(3.5f * width), blendMode = BlendMode.Plus)
                drawPath(path, lerp(color, Color.White, 0.75f).copy(alpha = 0.95f * fade), style = Stroke(1.4f * width), blendMode = BlendMode.Plus)
            }
        }
        val sparks = (6 + s.level * 30 * strength).toInt()
        repeat(sparks) {
            val angle = Random.nextFloat() * 2f * PI.toFloat()
            val r = r0 * (1.02f + Random.nextFloat() * 0.06f)
            val bend = angle + (Random.nextFloat() - 0.5f) * 0.15f
            val r2 = r + Random.nextFloat() * r0 * 0.14f
            drawLine(lerp(b, Color.White, 0.5f).copy(alpha = 0.5f), center + Offset(cos(angle), sin(angle)) * r, center + Offset(cos(bend), sin(bend)) * r2, 1.dp.toPx())
        }
    }

    // Láseres: rayos desde abajo y desde las esquinas de arriba que barren la pantalla al ritmo.
    fun lasers(scope: DrawScope) = with(scope) {
        val far = hypot(size.width, size.height) * 1.2f
        val scale = max(0.6f, min(size.width, size.height) / 500.dp.toPx() * 1.6f)
        val sources = listOf(
            Triple(Offset(size.width * 0.18f, size.height + 4), -PI.toFloat() / 2, 0.05f),
            Triple(Offset(size.width * 0.39f, size.height + 4), -PI.toFloat() / 2, 0.2f),
            Triple(Offset(size.width * 0.61f, size.height + 4), -PI.toFloat() / 2, 0.38f),
            Triple(Offset(size.width * 0.82f, size.height + 4), -PI.toFloat() / 2, 0.55f),
            Triple(Offset(-4f, -4f), PI.toFloat() / 4, 0.7f),
            Triple(Offset(size.width + 4, -4f), PI.toFloat() * 3 / 4, 0.8f),
        )
        for ((k, source) in sources.withIndex()) {
            val (at, base, band) = source
            val v = min(1f, s.at(band) * strength)
            val bright = (0.2f + v * 0.9f + s.kick * 0.4f * kickScale).coerceIn(0f, 1.2f)
            val sweep = sin(time * (0.7f + k * 0.13f) + k * 1.7f) * if (k < 4) 0.55f else 0.35f
            val spread = 0.06f + 0.16f * s.mid * strength
            val color = if (k % 2 == 1) b else a
            for (j in -1..1) {
                val angle = base + sweep + j * spread
                val end = at + Offset(cos(angle), sin(angle)) * far
                drawLine(color.copy(alpha = min(1f, 0.05f * bright)), at, end, 14 * scale, blendMode = BlendMode.Plus)
                drawLine(color.copy(alpha = min(1f, 0.2f * bright)), at, end, 4 * scale, blendMode = BlendMode.Plus)
                drawLine(lerp(color, Color.White, 0.6f).copy(alpha = min(1f, 0.9f * bright)), at, end, 1.3f * scale, blendMode = BlendMode.Plus)
            }
            drawCircle(Brush.radialGradient(listOf(lerp(color, Color.White, 0.5f).copy(alpha = 0.8f * min(1f, bright)), Color.Transparent), at, 30 * scale), 30 * scale, at)
        }
    }

    /** Un rayo quebrado: cada mitad se tuerce un poco, varias veces. */
    private fun jagged(from: Offset, to: Offset, bend: Float, depth: Int): List<Offset> {
        val out = mutableListOf(from)
        fun split(p: Offset, q: Offset, d: Float, left: Int) {
            if (left == 0) {
                out += q
                return
            }
            val mid = Offset((p.x + q.x) / 2 + (Random.nextFloat() - 0.5f) * d, (p.y + q.y) / 2 + (Random.nextFloat() - 0.5f) * d)
            split(p, mid, d / 2, left - 1)
            split(mid, q, d / 2, left - 1)
        }
        split(from, to, bend, depth)
        return out
    }
}
