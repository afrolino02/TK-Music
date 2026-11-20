package org.sonora.mobile

import android.os.Build
import android.os.SystemClock
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.TextFields
import androidx.compose.material3.AssistChip
import androidx.compose.material3.AssistChipDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.sonora_mobile.LyricLine
import uniffi.sonora_mobile.LyricWord
import uniffi.sonora_mobile.LyricsResult
import uniffi.sonora_mobile.Status
import kotlin.math.roundToInt

/** Cómo se anima la letra sincronizada. */
enum class LyricStyle(val label: String) {
    ENFOQUE("Enfoque"),
    KARAOKE("Karaoke"),
    CINEMATICA("Cinemática"),
    PALABRA("Palabra por palabra"),
    ESCRIBIR("Máquina de escribir"),
    CHAT("Chat"),
}

/** Qué va detrás de la letra. */
enum class LyricBackdrop(val label: String) {
    PORTADA("Portada desenfocada"),
    COLOR("Color de la portada"),
    VISUAL("Con el visual"),
}

/** Los dos colores de la portada que suena; los pone el reproductor. */
val LocalAccent = compositionLocalOf { Color.White to Color.White }

/** El azul de las burbujas propias del Chat y el amarillo de su palabra que suena. */
private val SentBlue = Color(0xFF3D7BF2)
private val SentWord = Color(0xFFFFE27A)

/**
 * Por dónde va la canción, en milisegundos, adelantado con el reloj entre los avisos del motor
 * (llegan cada medio segundo) para que las palabras se enciendan a tiempo.
 */
@Composable
fun rememberLyricPosition(): Long {
    val state = Store.state
    val reported = state?.positionMs?.toLong() ?: 0L
    val playing = state?.status == Status.PLAYING
    var anchor by remember { mutableStateOf(0L to 0L) }
    LaunchedEffect(reported, playing) { anchor = reported to SystemClock.uptimeMillis() }
    var tick by remember { mutableLongStateOf(SystemClock.uptimeMillis()) }
    LaunchedEffect(playing) {
        while (playing) {
            withFrameMillis { }
            tick = SystemClock.uptimeMillis()
        }
    }
    return if (playing) anchor.first + (tick - anchor.second).coerceIn(0, 1500) else reported
}

/**
 * Las palabras de una línea con sus tiempos. Casi ningún servicio los trae palabra por palabra:
 * entonces se reparten en la línea según el largo de cada palabra. Juntas forman el texto.
 */
fun timedWords(line: LyricLine, next: LyricLine?): List<LyricWord> {
    if (line.words.isNotEmpty()) return line.words
    if (line.text.isBlank()) return emptyList()
    val start = line.startMs.toLong()
    val end = (line.endMs?.toLong() ?: next?.startMs?.toLong() ?: (start + 4000)).coerceAtLeast(start + 400)
    val span = ((end - start) * 0.85).coerceAtMost(7000.0)
    val parts = Regex("\\S+\\s*").findAll(line.text).map { it.value }.toList()
    val weights = parts.map { it.trim().length + 1.5 }
    val total = weights.sum()
    var at = start.toDouble()
    return parts.mapIndexed { k, part ->
        val length = span * weights[k] / total
        val word = LyricWord(startMs = at.toLong().toULong(), endMs = (at + length).toLong().toULong(), text = part)
        at += length
        word
    }
}

/** Cuánto de una palabra ya se cantó, de 0 a 1. */
private fun progress(word: LyricWord, position: Long): Float {
    val start = word.startMs.toLong()
    val length = (word.endMs.toLong() - start).coerceAtLeast(1)
    return ((position - start).toFloat() / length).coerceIn(0f, 1f)
}

/** El color para resaltar, legible sobre el fondo del reproductor (claro u oscuro). */
@Composable
private fun highlight(): Color {
    val content = LocalContentColor.current
    if (Store.lyricWhite) return content
    val accent = LocalAccent.current.first
    val darkBackground = content.luminance() > 0.5f
    // sobre fondo oscuro siempre un tono claro, aunque la portada sea oscura
    return when {
        darkBackground -> {
            var light = accent
            var step = 0
            while (light.luminance() < 0.5f && step < 6) {
                light = lerp(light, Color.White, 0.25f)
                step++
            }
            light
        }
        accent.luminance() > 0.55f -> lerp(accent, Color.Black, 0.45f)
        else -> accent
    }
}

private fun textAlign() = if (Store.lyricStart) TextAlign.Start else TextAlign.Center
private fun horizontal() = if (Store.lyricStart) Alignment.Start else Alignment.CenterHorizontally

/** Las líneas que se repiten en la canción: casi siempre el coro. */
private fun chorusOf(result: LyricsResult): List<Boolean> {
    val counts = result.lines.groupingBy { it.text.trim().lowercase() }.eachCount()
    return result.lines.map { it.text.isNotBlank() && (counts[it.text.trim().lowercase()] ?: 0) >= 2 }
}

/**
 * La letra sincronizada con el estilo elegido en Ajustes. `big` es para TV y tablet. Enfoque es la
 * lista de siempre; los demás muestran la línea que suena con su animación.
 */
@Composable
fun StyledLyrics(result: LyricsResult, big: Boolean, focus: @Composable () -> Unit) {
    if (Store.lyricStyle == LyricStyle.ENFOQUE) {
        focus()
        return
    }
    val position = rememberLyricPosition()
    val active = result.lines.indexOfLast { it.startMs.toLong() <= position }
    val scale = Store.lyricSize * if (big) 1.3f else 1f
    when (Store.lyricStyle) {
        LyricStyle.KARAOKE -> Karaoke(result, position, active, scale)
        LyricStyle.CINEMATICA -> Cinematic(result, active, scale)
        LyricStyle.PALABRA -> WordByWord(result, position, active, scale)
        LyricStyle.ESCRIBIR -> Typewriter(result, position, active, scale)
        LyricStyle.CHAT -> Chat(result, position, active, scale)
        LyricStyle.ENFOQUE -> Unit
    }
}

/** El botón de la pantalla de letras para cambiar de estilo sin ir a Ajustes. */
@Composable
fun LyricStyleChip(modifier: Modifier = Modifier) {
    var open by remember { mutableStateOf(false) }
    Box(modifier) {
        AssistChip(
            onClick = { open = true },
            label = { Text(Store.lyricStyle.label) },
            leadingIcon = { Icon(Icons.Filled.TextFields, null, Modifier.size(AssistChipDefaults.IconSize)) },
            modifier = Modifier.focusRing(),
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            LyricStyle.entries.forEach { style ->
                DropdownMenuItem(
                    text = { Text(style.label) },
                    onClick = {
                        Store.chooseLyricStyle(style)
                        open = false
                    },
                    trailingIcon = { if (style == Store.lyricStyle) Icon(Icons.Filled.Check, null) },
                )
            }
        }
    }
}

/** Antes de la primera línea: tres puntos que se encienden mientras llega la voz. */
@Composable
private fun Waiting(result: LyricsResult, position: Long) {
    val first = result.lines.firstOrNull()?.startMs?.toLong() ?: 1L
    val lit = ((position.toFloat() / first.coerceAtLeast(1)) * 3.999f).toInt()
    val color = highlight()
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        repeat(3) { k ->
            Box(Modifier.size(10.dp).clip(CircleShape).background(color.copy(alpha = if (k <= lit) 1f else 0.25f)))
        }
    }
}

/** El contenedor de los estilos de una línea: centrado a lo alto, alineado según Ajustes. */
@Composable
private fun OneLine(content: @Composable () -> Unit) {
    Box(
        Modifier.fillMaxSize().padding(horizontal = 8.dp),
        contentAlignment = if (Store.lyricStart) Alignment.CenterStart else Alignment.Center,
    ) { content() }
}

// ── Karaoke ─────────────────────────────────────────────────────────────────────────────────

/** La frase que suena se pinta letra por letra con el color de la portada; la siguiente debajo. */
@Composable
private fun Karaoke(result: LyricsResult, position: Long, active: Int, scale: Float) {
    val accent = highlight()
    val dim = LocalContentColor.current.copy(alpha = 0.4f)
    OneLine {
        if (active < 0) {
            Waiting(result, position)
            return@OneLine
        }
        AnimatedContent(
            targetState = active,
            transitionSpec = {
                (fadeIn(tween(300)) + slideInVertically(tween(400)) { it / 3 }) togetherWith
                    (fadeOut(tween(200)) + slideOutVertically(tween(300)) { -it / 3 })
            },
            label = "karaoke",
        ) { index ->
            val line = result.lines[index]
            val words = remember(line) { timedWords(line, result.lines.getOrNull(index + 1)) }
            Column(horizontalAlignment = horizontal(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                val painted = buildAnnotatedString {
                    for (word in words) {
                        val lit = (word.text.trimEnd().length * progress(word, position)).roundToInt()
                        withStyle(SpanStyle(color = accent)) { append(word.text.substring(0, lit)) }
                        withStyle(SpanStyle(color = dim)) { append(word.text.substring(lit)) }
                    }
                }
                Text(
                    painted,
                    fontSize = (30 * scale).sp,
                    lineHeight = (38 * scale).sp,
                    fontWeight = FontWeight.ExtraBold,
                    textAlign = textAlign(),
                )
                result.lines.getOrNull(index + 1)?.let { next ->
                    Text(next.text, fontSize = (18 * scale).sp, fontWeight = FontWeight.Bold, color = dim, textAlign = textAlign())
                }
            }
        }
    }
}

// ── Cinemática ──────────────────────────────────────────────────────────────────────────────

/** Una sola frase grande en cursiva, como subtítulo: entra desenfocada desde abajo y sale hacia arriba. */
@Composable
private fun Cinematic(result: LyricsResult, active: Int, scale: Float) {
    OneLine {
        AnimatedContent(
            targetState = active,
            transitionSpec = {
                (fadeIn(tween(900)) + slideInVertically(tween(900, easing = FastOutSlowInEasing)) { it / 4 }) togetherWith
                    (fadeOut(tween(600)) + slideOutVertically(tween(700)) { -it / 4 })
            },
            label = "cinematic",
        ) { index ->
            val appear = remember { Animatable(0f) }
            LaunchedEffect(Unit) { appear.animateTo(1f, tween(1000, easing = FastOutSlowInEasing)) }
            val blur = ((1f - appear.value) * 14f).dp
            Text(
                if (index < 0) "♪" else result.lines[index].text.ifBlank { "♪" },
                fontSize = (34 * scale).sp,
                lineHeight = (42 * scale).sp,
                fontFamily = FontFamily.Serif,
                fontStyle = FontStyle.Italic,
                fontWeight = FontWeight.Medium,
                textAlign = textAlign(),
                modifier = Modifier.then(
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && blur > 0.3.dp) Modifier.blur(blur, BlurredEdgeTreatment.Unbounded) else Modifier,
                ),
            )
        }
    }
}

// ── Palabra por palabra ─────────────────────────────────────────────────────────────────────

/** Las palabras saltan a la pantalla una por una, con un rebote, justo cuando se cantan. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun WordByWord(result: LyricsResult, position: Long, active: Int, scale: Float) {
    val accent = highlight()
    val dim = LocalContentColor.current.copy(alpha = 0.4f)
    OneLine {
        if (active < 0) {
            Waiting(result, position)
            return@OneLine
        }
        AnimatedContent(
            targetState = active,
            transitionSpec = { fadeIn(tween(150)) togetherWith fadeOut(tween(250)) },
            label = "words",
        ) { index ->
            val line = result.lines[index]
            val words = remember(line) { timedWords(line, result.lines.getOrNull(index + 1)) }
            Column(horizontalAlignment = horizontal(), verticalArrangement = Arrangement.spacedBy(18.dp)) {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy((9 * scale).dp, horizontal()),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    for (word in words) {
                        val start = word.startMs.toLong()
                        PopWord(word.text.trim(), lit = position >= start, now = position >= start && position < word.endMs.toLong(), accent, scale)
                    }
                }
                result.lines.getOrNull(index + 1)?.let { next ->
                    Text(next.text, fontSize = (16 * scale).sp, fontWeight = FontWeight.Bold, color = dim, textAlign = textAlign())
                }
            }
        }
    }
}

@Composable
private fun PopWord(text: String, lit: Boolean, now: Boolean, accent: Color, scale: Float) {
    val shown by animateFloatAsState(
        if (lit) 1f else 0f,
        spring(dampingRatio = 0.42f, stiffness = 420f),
        label = "pop",
    )
    Text(
        text,
        fontSize = (32 * scale).sp,
        lineHeight = (40 * scale).sp,
        fontWeight = FontWeight.ExtraBold,
        color = if (now) accent else LocalContentColor.current,
        modifier = Modifier.graphicsLayer {
            alpha = shown.coerceIn(0f, 1f)
            val size = 0.55f + 0.45f * shown
            scaleX = size
            scaleY = size
            translationY = (1f - shown) * 18.dp.toPx()
        },
    )
}

// ── Máquina de escribir ─────────────────────────────────────────────────────────────────────

/** La frase se escribe letra por letra al ritmo del canto, con un cursor; la anterior arriba, apagada. */
@Composable
private fun Typewriter(result: LyricsResult, position: Long, active: Int, scale: Float) {
    val accent = highlight()
    val dim = LocalContentColor.current.copy(alpha = 0.4f)
    val blink = rememberInfiniteTransition(label = "caret")
    val caret by blink.animateFloat(1f, 0f, infiniteRepeatable(tween(500, easing = LinearEasing), RepeatMode.Reverse), label = "caret")
    OneLine {
        if (active < 0) {
            Waiting(result, position)
            return@OneLine
        }
        val line = result.lines[active]
        val words = remember(line) { timedWords(line, result.lines.getOrNull(active + 1)) }
        val typed = buildString {
            for (word in words) {
                val n = kotlin.math.ceil(progress(word, position) * word.text.length).toInt()
                append(word.text, 0, n)
                if (n < word.text.length) break
            }
        }
        Column(horizontalAlignment = horizontal(), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            result.lines.getOrNull(active - 1)?.let { previous ->
                Text(previous.text, fontSize = (15 * scale).sp, fontFamily = FontFamily.Monospace, color = dim, textAlign = textAlign())
            }
            Text(
                buildAnnotatedString {
                    append(typed.trimEnd())
                    withStyle(SpanStyle(color = accent.copy(alpha = caret))) { append("▌") }
                },
                fontSize = (24 * scale).sp,
                lineHeight = (32 * scale).sp,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Medium,
                textAlign = textAlign(),
            )
        }
    }
}

// ── Chat ────────────────────────────────────────────────────────────────────────────────────

/** Lugares conocidos que, si la frase los nombra, llegan con su tarjeta de mapa. */
private val PLACES = listOf(
    "cartagena" to "Cartagena, Colombia", "barranquilla" to "Barranquilla, Colombia", "medellín" to "Medellín, Colombia",
    "medellin" to "Medellín, Colombia", "bogotá" to "Bogotá, Colombia", "bogota" to "Bogotá, Colombia", "cali" to "Cali, Colombia",
    "santa marta" to "Santa Marta, Colombia", "san andrés" to "San Andrés, Colombia", "puerto rico" to "Puerto Rico",
    "san juan" to "San Juan, Puerto Rico", "la habana" to "La Habana, Cuba", "santo domingo" to "Santo Domingo, R. D.",
    "caracas" to "Caracas, Venezuela", "panamá" to "Panamá", "ciudad de méxico" to "Ciudad de México", "méxico" to "México",
    "buenos aires" to "Buenos Aires, Argentina", "lima" to "Lima, Perú", "santiago" to "Santiago, Chile", "madrid" to "Madrid, España",
    "barcelona" to "Barcelona, España", "ibiza" to "Ibiza, España", "parís" to "París, Francia", "paris" to "París, Francia",
    "londres" to "Londres, Reino Unido", "london" to "London, UK", "tokyo" to "Tokio, Japón", "tokio" to "Tokio, Japón",
    "new york" to "New York, USA", "nueva york" to "Nueva York, USA", "brooklyn" to "Brooklyn, New York", "manhattan" to "Manhattan, New York",
    "hollywood" to "Hollywood, Los Angeles", "los angeles" to "Los Angeles, USA", "miami" to "Miami, USA", "las vegas" to "Las Vegas, USA",
    "chicago" to "Chicago, USA", "atlanta" to "Atlanta, USA", "houston" to "Houston, USA", "toronto" to "Toronto, Canadá",
    "malecón" to "El Malecón", "malecon" to "El Malecón",
)

private fun placeIn(text: String): String? {
    val lower = " " + text.lowercase() + " "
    return PLACES.firstOrNull { (word, _) -> Regex("[^\\p{L}]" + Regex.escape(word) + "[^\\p{L}]").containsMatchIn(lower) }?.second
}

/**
 * La letra como mensajes que flotan al centro: los «escribiendo…» antes de cada frase, la
 * burbuja con las palabras encendiéndose y las viejas que suben, se apagan y se desenfocan. El
 * coro (las líneas que se repiten) llega del otro lado, en azul.
 */
@Composable
private fun Chat(result: LyricsResult, position: Long, active: Int, scale: Float) {
    val chorus = remember(result) { chorusOf(result) }
    val shown = active + 1
    val next = result.lines.getOrNull(shown)
    val typing = next != null && !chorus[shown] && next.startMs.toLong() - position in 0..1100
    // de la más nueva a la más vieja: la lista va de abajo hacia arriba
    val rows = buildList {
        if (typing) add(-1)
        for (j in (shown - 1) downTo maxOf(0, shown - 7)) add(j)
    }
    BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        LazyColumn(
            modifier = Modifier.widthIn(max = 460.dp).fillMaxWidth().fillMaxHeight(0.78f),
            reverseLayout = true,
            userScrollEnabled = false,
            verticalArrangement = Arrangement.spacedBy(6.dp),
            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
        ) {
            items(rows, key = { it }) { j ->
                val age = if (j < 0) 0 else shown - 1 - j
                Box(Modifier.animateItem()) {
                    if (j < 0) {
                        Typing()
                    } else {
                        val line = result.lines[j]
                        Bubble(
                            line = line,
                            words = remember(line) { timedWords(line, result.lines.getOrNull(j + 1)) },
                            out = chorus[j],
                            age = age,
                            position = if (j == active) position else Long.MAX_VALUE,
                            scale = scale,
                        )
                    }
                }
            }
        }
    }
}

private val FADE = floatArrayOf(1f, 0.9f, 0.62f, 0.36f, 0.18f, 0.08f, 0f)
private val BLUR = floatArrayOf(0f, 0f, 1f, 2f, 3f, 4f, 5f)

@Composable
private fun Bubble(line: LyricLine, words: List<LyricWord>, out: Boolean, age: Int, position: Long, scale: Float) {
    val appear = remember { Animatable(0f) }
    LaunchedEffect(Unit) { appear.animateTo(1f, spring(dampingRatio = 0.55f, stiffness = 380f)) }
    val step = age.coerceAtMost(FADE.size - 1)
    val fade by animateFloatAsState(FADE[step], tween(450), label = "fade")
    val blur by animateFloatAsState(BLUR[step], tween(450), label = "blur")
    val accent = highlight()
    val place = remember(line) { placeIn(line.text) }
    val side = if (out) 1f else -1f
    val shape = RoundedCornerShape(
        topStart = 18.dp, topEnd = 18.dp,
        bottomStart = if (out) 18.dp else 5.dp, bottomEnd = if (out) 5.dp else 18.dp,
    )
    val text = buildAnnotatedString {
        val now = if (out) SentWord else accent
        for (word in words) {
            val start = word.startMs.toLong()
            val color = when {
                position >= start && position < word.endMs.toLong() -> now
                position >= start -> Color.White
                else -> Color.White.copy(alpha = 0.42f)
            }
            withStyle(SpanStyle(color = color)) { append(word.text) }
        }
        if (words.isEmpty()) append(line.text.ifBlank { "♪" })
    }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = if (out) Arrangement.End else Arrangement.Start) {
        Column(
            Modifier
                .widthIn(max = 320.dp)
                .graphicsLayer {
                    val a = appear.value
                    alpha = fade * a.coerceIn(0f, 1f)
                    val size = (0.5f + 0.5f * a) * (1f - step * 0.025f)
                    scaleX = size
                    scaleY = size
                    translationX = side * (1f - a) * 40.dp.toPx()
                    rotationZ = side * (1f - a) * 7f
                    transformOrigin = androidx.compose.ui.graphics.TransformOrigin(if (out) 1f else 0f, 1f)
                }
                .then(if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && blur > 0.2f) Modifier.blur(blur.dp, BlurredEdgeTreatment.Unbounded) else Modifier)
                .clip(shape)
                .background(if (out) SentBlue else Color(0xFF2A2830)),
        ) {
            if (place != null) MapCard()
            Text(
                text,
                fontSize = (16 * scale).sp,
                lineHeight = (21 * scale).sp,
                fontWeight = if (place != null) FontWeight.SemiBold else FontWeight.Medium,
                modifier = Modifier.padding(horizontal = 13.dp, vertical = 8.dp),
            )
            if (place != null) {
                Text(
                    "📍 $place",
                    fontSize = (11 * scale).sp,
                    color = Color.White.copy(alpha = 0.75f),
                    modifier = Modifier.padding(start = 13.dp, end = 13.dp, bottom = 9.dp),
                )
            }
        }
    }
}

/** Un mapa dibujado (calles, agua y un alfiler), no uno de verdad. */
@Composable
private fun MapCard() {
    val pin = highlight()
    Canvas(Modifier.widthIn(min = 220.dp).fillMaxWidth().height(64.dp).background(Color(0xFF1B2638))) {
        val w = size.width
        val h = size.height
        val water = androidx.compose.ui.graphics.Path().apply {
            moveTo(0f, 0f); lineTo(w * 0.28f, 0f)
            cubicTo(w * 0.22f, h * 0.3f, w * 0.32f, h * 0.6f, w * 0.2f, h); lineTo(0f, h); close()
        }
        drawPath(water, Color(0xFF163556))
        val road = Color(0xFF30415C)
        drawLine(road, Offset(w * 0.3f, h * 0.15f), Offset(w, h * 0.42f), 6f)
        drawLine(road, Offset(w * 0.42f, 0f), Offset(w * 0.54f, h), 6f)
        drawLine(road, Offset(w * 0.67f, 0f), Offset(w * 0.75f, h), 6f)
        drawLine(road, Offset(w * 0.29f, h * 0.72f), Offset(w, h * 0.66f), 6f)
        drawLine(Color(0xFF4A5B78), Offset(w * 0.48f, h * 0.25f), Offset(w, h * 0.9f), 3f)
        drawCircle(pin, radius = 11f, center = Offset(w * 0.6f, h * 0.5f))
        drawCircle(Color.White, radius = 4f, center = Offset(w * 0.6f, h * 0.5f))
    }
}

@Composable
private fun Typing() {
    val dots = rememberInfiniteTransition(label = "typing")
    Row(Modifier.fillMaxWidth()) {
        Row(
            Modifier.clip(RoundedCornerShape(18.dp, 18.dp, 18.dp, 5.dp)).background(Color(0xFF2A2830)).padding(horizontal = 14.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(5.dp),
        ) {
            repeat(3) { k ->
                val lift by dots.animateFloat(
                    0f, 1f,
                    infiniteRepeatable(tween(500, delayMillis = k * 150, easing = FastOutSlowInEasing), RepeatMode.Reverse),
                    label = "dot$k",
                )
                Box(
                    Modifier
                        .size(7.dp)
                        .graphicsLayer { translationY = -lift * 4.dp.toPx(); alpha = 0.5f + 0.5f * lift }
                        .clip(CircleShape)
                        .background(Color.White.copy(alpha = 0.8f)),
                )
            }
        }
    }
}

// ── La línea que suena, fuera de la pantalla de letras ──────────────────────────────────────

/** La letra de lo que suena, para la pantalla completa y la barra de abajo. */
@Composable
fun rememberCurrentLyrics(): LyricsResult? {
    val state = Store.state
    val track = state?.track
    val slug = state?.provider
    val key = if (track != null && slug != null) "$slug:${track.id}" else null
    var result by remember(key) { mutableStateOf<LyricsResult?>(null) }
    LaunchedEffect(key) {
        if (track != null && slug != null) Store.run { it.lyricsFor(slug, track) }.onSuccess { result = it }
    }
    return result
}

/** La frase pintada hasta donde va la voz: lo cantado en `lit`, lo que falta en `dim`. */
private fun painted(words: List<LyricWord>, position: Long, lit: Color, dim: Color): AnnotatedString = buildAnnotatedString {
    for (word in words) {
        val n = (word.text.trimEnd().length * progress(word, position)).roundToInt()
        withStyle(SpanStyle(color = lit)) { append(word.text.substring(0, n)) }
        withStyle(SpanStyle(color = dim)) { append(word.text.substring(n)) }
    }
}

/**
 * La frase que suena, grande, encima del visual de la pantalla completa. Se pinta con el color
 * de la portada mientras se canta y cambia con un fundido.
 */
@Composable
fun StageLine(modifier: Modifier = Modifier, big: Boolean) {
    val result = rememberCurrentLyrics() ?: return
    if (!result.synced || result.lines.isEmpty()) return
    val position = rememberLyricPosition()
    val active = result.lines.indexOfLast { it.startMs.toLong() <= position }
    val accent = highlight()
    val dim = LocalContentColor.current.copy(alpha = 0.55f)
    val scale = Store.lyricSize * if (big) 1.2f else 1f
    Box(modifier, contentAlignment = Alignment.Center) {
        AnimatedContent(
            targetState = active,
            transitionSpec = {
                (fadeIn(tween(400)) + slideInVertically(tween(450)) { it / 3 }) togetherWith
                    (fadeOut(tween(250)) + slideOutVertically(tween(350)) { -it / 3 })
            },
            label = "stage-line",
        ) { index ->
            val line = result.lines.getOrNull(index)
            if (line == null || line.text.isBlank()) return@AnimatedContent
            val words = remember(line) { timedWords(line, result.lines.getOrNull(index + 1)) }
            Text(
                painted(words, position, accent, dim),
                fontSize = (26 * scale).sp,
                lineHeight = (33 * scale).sp,
                fontWeight = FontWeight.ExtraBold,
                textAlign = TextAlign.Center,
                maxLines = 2,
            )
        }
    }
}

/**
 * La segunda línea del mini reproductor: la frase que suena si hay letra sincronizada y está
 * activado en Ajustes; si no, `fallback` (el artista).
 */
@Composable
fun BarLine(fallback: String, playing: Boolean) {
    val small = androidx.compose.material3.MaterialTheme.typography.bodySmall
    val muted = androidx.compose.material3.MaterialTheme.colorScheme.onSurfaceVariant
    val plain = @Composable {
        Text(fallback, style = small, color = muted, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
    }
    val result = if (Store.lyricsInBar && playing) rememberCurrentLyrics() else null
    if (result == null || !result.synced || result.lines.isEmpty()) return plain()
    val position = rememberLyricPosition()
    val active = result.lines.indexOfLast { it.startMs.toLong() <= position }
    val line = result.lines.getOrNull(active)
    if (line == null || line.text.isBlank()) return plain()
    val words = remember(line) { timedWords(line, result.lines.getOrNull(active + 1)) }
    Text(
        painted(words, position, androidx.compose.material3.MaterialTheme.colorScheme.onSurface, muted),
        style = small,
        maxLines = 1,
        overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
    )
}
