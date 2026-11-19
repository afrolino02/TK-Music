package org.sonora.mobile

import android.content.Context
import android.content.Intent
import android.graphics.PixelFormat
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.view.Gravity
import android.view.WindowManager
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.setViewTreeLifecycleOwner
import androidx.savedstate.SavedStateRegistry
import androidx.savedstate.SavedStateRegistryController
import androidx.savedstate.SavedStateRegistryOwner
import androidx.savedstate.setViewTreeSavedStateRegistryOwner
import uniffi.sonora_mobile.Status

/**
 * El dock: una barra chica que flota encima de las otras apps con la frase que suena. Va apagado
 * hasta que se activa en Ajustes (necesita el permiso «Mostrar sobre otras apps»). Para no chocar
 * con la pastilla de música que algunos Android ya ponen arriba, sale a la derecha, se puede
 * arrastrar a donde estorbe menos, se esconde con TK-Music abierta y no se ve en pausa.
 */
object Dock {
    private const val PREFS = "dock"
    private var view: ComposeView? = null
    private var owner: OverlayOwner? = null
    private var params: WindowManager.LayoutParams? = null

    /** Si el teléfono deja dibujar encima de otras apps. */
    fun allowed(context: Context): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.M || Settings.canDrawOverlays(context)

    /** Lleva a la pantalla de Android donde se da el permiso. */
    fun askPermission(context: Context) {
        val intent = Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:${context.packageName}"))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(intent) }
    }

    /** TK-Music pasó a segundo plano: si el dock está activado y hay permiso, aparece. */
    fun show(context: Context) {
        if (!Store.dockEnabled || !allowed(context) || view != null) return
        val app = context.applicationContext
        val windows = app.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        val prefs = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val density = app.resources.displayMetrics.density
        val layout = WindowManager.LayoutParams(
            WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.WRAP_CONTENT,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
            } else {
                @Suppress("DEPRECATION")
                WindowManager.LayoutParams.TYPE_PHONE
            },
            // los toques fuera del dock siguen a la app de abajo
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.END
            x = prefs.getInt("x", (12 * density).toInt())
            // por debajo de la barra de estado, donde no tapa la isla o la pastilla del sistema
            y = prefs.getInt("y", (56 * density).toInt())
        }
        val lifecycle = OverlayOwner().also { it.start() }
        val compose = ComposeView(app).apply {
            setViewTreeLifecycleOwner(lifecycle)
            setViewTreeSavedStateRegistryOwner(lifecycle)
            setContent {
                DockContent(onDrag = { dx, dy ->
                    layout.x = (layout.x - dx.toInt()).coerceAtLeast(0)
                    layout.y = (layout.y + dy.toInt()).coerceAtLeast(0)
                    runCatching { windows.updateViewLayout(this, layout) }
                }, onDrop = {
                    prefs.edit().putInt("x", layout.x).putInt("y", layout.y).apply()
                })
            }
        }
        if (runCatching { windows.addView(compose, layout) }.isSuccess) {
            view = compose
            owner = lifecycle
            params = layout
        } else {
            lifecycle.stop()
        }
    }

    /** TK-Music volvió al frente (o se apagó el dock): se quita. */
    fun hide(context: Context) {
        val current = view ?: return
        val windows = context.applicationContext.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        runCatching { windows.removeView(current) }
        owner?.stop()
        view = null
        owner = null
        params = null
    }
}

/** Lo mínimo que Compose necesita para vivir fuera de una pantalla de la app. */
internal class OverlayOwner : LifecycleOwner, SavedStateRegistryOwner {
    private val registry = LifecycleRegistry(this)
    private val saved = SavedStateRegistryController.create(this)
    override val lifecycle: Lifecycle get() = registry
    override val savedStateRegistry: SavedStateRegistry get() = saved.savedStateRegistry

    fun start() {
        saved.performRestore(null)
        registry.currentState = Lifecycle.State.RESUMED
    }

    fun stop() {
        registry.currentState = Lifecycle.State.DESTROYED
    }
}

/**
 * Cerrado: la portada que gira, la frase que suena y unas barritas. Abierto (un toque): la
 * portada, el título, los controles y tres líneas de letra. Se arrastra para moverlo.
 */
@Composable
private fun DockContent(onDrag: (Float, Float) -> Unit, onDrop: () -> Unit) {
    val state = Store.state ?: return
    val track = state.track ?: return
    // en pausa no se ve, para no estorbar
    if (state.status != Status.PLAYING && state.status != Status.LOADING) return
    var open by remember { mutableStateOf(false) }
    val lyrics = rememberCurrentLyrics()
    val position = rememberLyricPosition()
    val lines = lyrics?.takeIf { it.synced }?.lines.orEmpty()
    val active = lines.indexOfLast { it.startMs.toLong() <= position }
    val line = lines.getOrNull(active)?.text?.takeIf { it.isNotBlank() }
    MaterialTheme(colorScheme = darkColorScheme()) {
        CompositionLocalProvider(LocalContentColor provides Color.White) {
            Box(
                Modifier
                    .padding(6.dp)
                    .clip(RoundedCornerShape(if (open) 26.dp else 22.dp))
                    .background(Color(0xEB0A090E))
                    .pointerInput(Unit) {
                        detectDragGestures(onDragEnd = onDrop) { change, drag ->
                            change.consume()
                            onDrag(drag.x, drag.y)
                        }
                    }
                    .clickable { open = !open }
                    .animateContentSize(spring(dampingRatio = 0.7f, stiffness = 500f)),
            ) {
                if (!open) {
                    Row(
                        Modifier.height(40.dp).widthIn(max = 280.dp).padding(start = 6.dp, end = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Cover(track.cover, 28.dp, round = true)
                        Text(
                            line ?: track.title,
                            fontSize = 13.sp,
                            fontWeight = FontWeight.SemiBold,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                        Bars()
                    }
                } else {
                    Column(Modifier.width(300.dp).padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                            Cover(track.cover, 56.dp)
                            Column(Modifier.weight(1f)) {
                                Text(track.title, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(track.artists, fontSize = 12.sp, color = Color.White.copy(alpha = 0.6f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Spacer(Modifier.weight(1f))
                            IconButton(onClick = { Store.engine.previous() }) { Icon(Icons.Filled.SkipPrevious, "Anterior") }
                            IconButton(onClick = { Store.engine.toggle() }) {
                                Icon(if (state.status == Status.PLAYING) Icons.Filled.Pause else Icons.Filled.PlayArrow, "Pausar o seguir")
                            }
                            IconButton(onClick = { Store.engine.next() }) { Icon(Icons.Filled.SkipNext, "Siguiente") }
                            Spacer(Modifier.weight(1f))
                        }
                        if (lines.isNotEmpty()) {
                            lines.getOrNull(active - 1)?.let { Text(it.text, fontSize = 12.sp, color = Color.White.copy(alpha = 0.4f), maxLines = 1, overflow = TextOverflow.Ellipsis) }
                            Text(line ?: "♪", fontSize = 16.sp, fontWeight = FontWeight.Bold)
                            lines.getOrNull(active + 1)?.let { Text(it.text, fontSize = 12.sp, color = Color.White.copy(alpha = 0.4f), maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        }
                    }
                }
            }
        }
    }
}

/** Unas barritas que se mueven mientras suena. */
@Composable
private fun Bars() {
    val beat = androidx.compose.animation.core.rememberInfiniteTransition(label = "bars")
    Row(Modifier.height(14.dp), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.Bottom) {
        repeat(4) { k ->
            val level by beat.animateFloat(
                0.25f, 1f,
                androidx.compose.animation.core.infiniteRepeatable(
                    androidx.compose.animation.core.tween(380 + k * 90),
                    androidx.compose.animation.core.RepeatMode.Reverse,
                ),
                label = "bar$k",
            )
            Box(Modifier.width(3.dp).fillMaxHeight(level).clip(RoundedCornerShape(2.dp)).background(Color(0xFFFF7A45)))
        }
    }
}
