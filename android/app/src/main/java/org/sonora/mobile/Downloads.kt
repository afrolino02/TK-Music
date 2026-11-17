@file:OptIn(ExperimentalMaterial3Api::class)

package org.sonora.mobile

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import uniffi.sonora_mobile.TrackItem
import java.text.Normalizer

/** Las canciones descargadas a propósito para escucharlas sin internet, con un buscador. */
@Composable
fun DownloadsScreen() {
    var query by rememberSaveable { mutableStateOf("") }
    Column(Modifier.fillMaxSize()) {
        Bar("Sin conexión", back = true)
        Loader(key = "downloads-${Store.downloadRevision}", load = { it.downloadedTracks("youtube") }) { all ->
            val tracks = filterTracks(all, query)
            LazyColumn(Modifier.fillMaxSize()) {
                if (all.isNotEmpty()) {
                    item { FilterField(query, "Buscar en sin conexión") { query = it } }
                    item { PlayButtons(tracks, "youtube") }
                }
                when {
                    all.isEmpty() -> item {
                        Hint(
                            "Todavía no hay nada guardado para sin conexión. Con internet, toca ⋮ en una canción y elige «Guardar para sin conexión», o el botón de descarga de una playlist o álbum.",
                        )
                    }
                    tracks.isEmpty() -> item { Hint("Nada guardado sin conexión coincide con «${query.trim()}».") }
                    else -> {
                        item {
                            Text(
                                if (tracks.size == 1) "1 canción" else "${tracks.size} canciones",
                                style = MaterialTheme.typography.labelLarge,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(start = 16.dp, top = 4.dp, bottom = 4.dp),
                            )
                        }
                        trackItems(tracks, "youtube")
                    }
                }
                item { Spacer(Modifier.size(96.dp)) }
            }
        }
    }
}

/** La búsqueda sin conexión: entre lo descargado y lo ya escuchado que quedó guardado. */
@Composable
fun OfflineSearch() {
    var query by rememberSaveable { mutableStateOf("") }
    Column(Modifier.fillMaxSize()) {
        Bar("Buscar")
        FilterField(query, "Buscar en tu música guardada") { query = it }
        Text(
            "Sin internet: se busca en tu música descargada y en lo guardado para sin conexión.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp),
        )
        val phone = Store.signedIn.any { it.slug == "local" }
        Loader(key = "offline-search-${Store.downloadRevision}-${Store.libraryRevision}-$phone", load = { engine ->
            Pair(
                if (phone) runCatching { engine.libraryTracks("local") }.getOrDefault(emptyList()) else emptyList(),
                engine.offlineTracks(),
            )
        }) { (files, saved) ->
            val local = filterTracks(files, query)
            val kept = filterTracks(saved, query)
            LazyColumn(Modifier.fillMaxSize()) {
                if (files.isEmpty() && saved.isEmpty()) {
                    item { Hint("Todavía no hay canciones en el teléfono.") }
                } else if (local.isEmpty() && kept.isEmpty()) {
                    item { Hint("Nada en el teléfono coincide con «${query.trim()}».") }
                }
                if (local.isNotEmpty()) {
                    item { SectionTitle("Música descargada") }
                    trackItems(local, "local")
                }
                if (kept.isNotEmpty()) {
                    item { SectionTitle("Sin conexión") }
                    trackItems(kept, "youtube")
                }
                item { Spacer(Modifier.size(96.dp)) }
            }
        }
    }
}

/** Siluetas de tarjetas que laten mientras llega el inicio, en vez de un círculo girando. */
@Composable
fun HomeSkeleton() {
    val pulse by rememberInfiniteTransition(label = "skeleton").animateFloat(
        initialValue = 0.35f,
        targetValue = 0.7f,
        animationSpec = infiniteRepeatable(tween(900), RepeatMode.Reverse),
        label = "skeleton-alpha",
    )
    val tone = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.12f)
    Column(Modifier.fillMaxWidth().alpha(pulse)) {
        Block(tone, Modifier.padding(horizontal = 16.dp, vertical = 8.dp).fillMaxWidth().height(210.dp), 20.dp)
        repeat(3) {
            Block(tone, Modifier.padding(start = 16.dp, top = 20.dp, bottom = 10.dp).width(160.dp).height(18.dp), 6.dp)
            Row(Modifier.padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                repeat(4) {
                    Column {
                        Block(tone, Modifier.size(124.dp), 10.dp)
                        Block(tone, Modifier.padding(top = 8.dp).width(96.dp).height(12.dp), 4.dp)
                    }
                }
            }
        }
    }
}

@Composable
private fun Block(color: androidx.compose.ui.graphics.Color, modifier: Modifier, corner: Dp) {
    Box(modifier.clip(RoundedCornerShape(corner)).background(color))
}

@Composable
private fun FilterField(query: String, hint: String, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = query,
        onValueChange = onChange,
        placeholder = { Text(hint) },
        leadingIcon = { Icon(Icons.Filled.Search, null) },
        trailingIcon = {
            if (query.isNotEmpty()) IconButton(onClick = { onChange("") }) { Icon(Icons.Filled.Clear, "Borrar") }
        },
        singleLine = true,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun Hint(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        fontWeight = FontWeight.Normal,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(16.dp),
    )
}

/** Las canciones cuyo título, artista o álbum contienen todas las palabras, sin mirar tildes. */
fun filterTracks(tracks: List<TrackItem>, query: String): List<TrackItem> {
    val words = plain(query).split(' ').filter { it.isNotBlank() }
    if (words.isEmpty()) return tracks
    return tracks.filter { track ->
        val text = plain("${track.title} ${track.artists} ${track.album}")
        words.all { it in text }
    }
}

private fun plain(text: String): String =
    Normalizer.normalize(text.lowercase(), Normalizer.Form.NFD).replace(Regex("\\p{Mn}+"), "")
