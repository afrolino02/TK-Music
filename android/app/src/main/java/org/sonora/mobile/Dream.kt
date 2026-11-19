package org.sonora.mobile

import android.service.dreams.DreamService
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.setViewTreeLifecycleOwner
import androidx.savedstate.setViewTreeSavedStateRegistryOwner

/**
 * El protector de pantalla de Android TV: el visual elegido con la portada, el título y la
 * frase que suena. Se elige en los ajustes de la TV (Protector de pantalla → TK-Music).
 */
class TKMusicDream : DreamService() {
    private var owner: OverlayOwner? = null

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        isInteractive = false
        isFullscreen = true
        val lifecycle = OverlayOwner().also { it.start() }
        owner = lifecycle
        setContentView(
            ComposeView(this).apply {
                setViewTreeLifecycleOwner(lifecycle)
                setViewTreeSavedStateRegistryOwner(lifecycle)
                setContent { DreamContent() }
            },
        )
    }

    override fun onDetachedFromWindow() {
        owner?.stop()
        owner = null
        super.onDetachedFromWindow()
    }
}

/** Los colores del protector cuando no hay portada de donde sacarlos. */
private val DreamGlow = Color(0xFFFF7A45)
private val DreamDepth = Color(0xFF8B5CF6)

@Composable
private fun DreamContent() {
    MaterialTheme(colorScheme = darkColorScheme()) {
        CompositionLocalProvider(LocalContentColor provides Color.White, LocalAccent provides (DreamGlow to DreamDepth)) {
            val spectrum = rememberSpectrum()
            val style = Store.visualStyle
            val track = Store.state?.track
            BoxWithConstraints(Modifier.fillMaxSize().background(Color.Black)) {
                val cover = minOf(maxWidth * 0.3f, maxHeight * 0.45f)
                if (style == VisualStyle.ONDAS) {
                    StageWave(DreamGlow, DreamDepth, Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(maxHeight * 0.3f))
                } else {
                    StageVisual(style, DreamGlow, DreamDepth, spectrum, focus = maxWidth / 2 to maxHeight * 0.4f - 24.dp, cover = cover, modifier = Modifier.fillMaxSize())
                }
                Column(
                    Modifier.align(Alignment.Center).padding(bottom = maxHeight * 0.1f),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    if (track != null) {
                        Box(Modifier.graphicsLayer { pulseScale(style, spectrum).let { scaleX = it; scaleY = it } }) {
                            Cover(track.cover, cover)
                        }
                        Text(track.title, fontSize = 26.sp, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center)
                        Text(track.artists, fontSize = 16.sp, color = Color.White.copy(alpha = 0.6f), textAlign = TextAlign.Center)
                    } else {
                        Text("TK-Music", fontSize = 34.sp, fontWeight = FontWeight.Bold)
                    }
                }
                if (track != null) {
                    StageLine(Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(bottom = maxHeight * 0.08f).padding(horizontal = 48.dp), big = true)
                }
            }
        }
    }
}
