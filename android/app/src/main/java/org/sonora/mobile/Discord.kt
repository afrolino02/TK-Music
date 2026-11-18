package org.sonora.mobile

import android.content.Context
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import uniffi.sonora_mobile.TrackItem
import java.net.HttpURLConnection
import java.net.URL

/**
 * El puente con el bot de Discord de TK-Music: la canción se manda, por un webhook, al canal
 * #tk-music del servidor, y el bot la agrega a su cola. Funciona desde cualquier lado, sin
 * estar en la misma red que el PC del bot.
 */
object Discord {
    /** La dirección del webhook del canal #tk-music, o "" si no se configuró. */
    var webhook by mutableStateOf("")
        private set

    fun load(context: Context) {
        webhook = prefs(context).getString("discord.webhook", "") ?: ""
    }

    fun save(context: Context, url: String) {
        webhook = url.trim()
        prefs(context).edit().putString("discord.webhook", webhook).apply()
    }

    /** Solo las canciones de YouTube Music tienen un enlace que el bot sepa abrir. */
    fun canSend(slug: String) = webhook.isNotEmpty() && slug == "youtube"

    /** Manda la canción al canal del bot. */
    fun send(track: TrackItem) {
        val url = webhook
        Store.scope.launch {
            val sent = withContext(Dispatchers.IO) {
                runCatching {
                    val body = JSONObject()
                        .put("content", "https://music.youtube.com/watch?v=${track.id}")
                        .put("username", "TK-Music · ${Store.userName.ifBlank { "teléfono" }}")
                        .toString()
                    val connection = (URL(url).openConnection() as HttpURLConnection).apply {
                        requestMethod = "POST"
                        doOutput = true
                        connectTimeout = 10_000
                        readTimeout = 10_000
                        setRequestProperty("Content-Type", "application/json")
                    }
                    connection.outputStream.use { it.write(body.toByteArray()) }
                    val code = connection.responseCode
                    connection.disconnect()
                    code in 200..299
                }.getOrDefault(false)
            }
            Store.problem = when (sent) {
                true -> "Enviada al bot de Discord: ${track.title}"
                false -> "No se pudo enviar al bot. Revisa el webhook en Ajustes y tu conexión."
            }
        }
    }

    private fun prefs(context: Context) = context.getSharedPreferences("sonora", Context.MODE_PRIVATE)
}

/** «Enviar al bot de Discord», en el menú de una canción. */
@Composable
fun DiscordMenuItem(track: TrackItem, slug: String, onDismiss: () -> Unit) {
    if (!Discord.canSend(slug)) return
    DropdownMenuItem(
        text = { Text("Enviar al bot de Discord") },
        leadingIcon = { Icon(Icons.AutoMirrored.Filled.Send, null) },
        onClick = {
            onDismiss()
            Discord.send(track)
        },
    )
}

/** La sección de Ajustes donde se pega el webhook del canal #tk-music. */
@Composable
fun DiscordSection() {
    val context = LocalContext.current
    var editing by remember { mutableStateOf(false) }
    var draft by remember { mutableStateOf(Discord.webhook) }
    ListItem(
        headlineContent = { Text("Bot de Discord") },
        supportingContent = {
            Text(
                if (Discord.webhook.isEmpty()) "Pega el webhook del canal #tk-music para mandar canciones al bot"
                else "Conectado: en el menú ⋮ de cada canción sale «Enviar al bot de Discord»",
            )
        },
        leadingContent = { Icon(Icons.AutoMirrored.Filled.Send, null) },
        modifier = Modifier.focusRing().clickable { draft = Discord.webhook; editing = !editing },
    )
    if (!editing) return
    Column(Modifier.padding(horizontal = 16.dp)) {
        Text(
            "En Discord: ajustes del canal #tk-music → Integraciones → Webhooks → Nuevo webhook → Copiar URL del webhook.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedTextField(
            value = draft,
            onValueChange = { draft = it },
            placeholder = { Text("https://discord.com/api/webhooks/…") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
        )
        Row {
            TextButton(onClick = {
                val url = draft.trim()
                if (url.isEmpty() || url.startsWith("https://discord.com/api/webhooks/") ||
                    url.startsWith("https://discordapp.com/api/webhooks/")
                ) {
                    Discord.save(context, url)
                    editing = false
                    Store.problem = if (url.isEmpty()) "Bot de Discord desconectado." else "Webhook guardado."
                } else {
                    Store.problem = "Eso no parece un webhook de Discord: empieza por https://discord.com/api/webhooks/"
                }
            }) { Text("Guardar") }
            if (Discord.webhook.isNotEmpty()) {
                TextButton(onClick = { Discord.save(context, ""); draft = ""; editing = false }) { Text("Quitar") }
            }
        }
    }
}
