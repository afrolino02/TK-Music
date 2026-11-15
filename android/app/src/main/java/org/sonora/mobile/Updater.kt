package org.sonora.mobile

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/**
 * Las actualizaciones de TK-Music: al abrir, mira la última versión publicada en el repo
 * (GitHub Releases) y, si es más nueva que la instalada, ofrece bajarla e instalarla encima.
 * Android siempre pide confirmar la instalación; eso no se puede saltar fuera de Play Store.
 */
object Updater {
    private const val LATEST = "https://api.github.com/repos/rprofe358/TK-Music/releases/latest"
    private const val APK = "TK-Music.apk"

    /** Una versión publicada que se puede instalar. */
    data class Release(val version: String, val notes: String, val url: String)

    /** La versión nueva que se está ofreciendo, o null si no hay (o la persona dijo «ahora no»). */
    var offered by mutableStateOf<Release?>(null)
        private set

    /** Lo descargado (0..1) mientras baja el APK; null si no está bajando. */
    var progress by mutableStateOf<Float?>(null)
        private set

    /** Un problema al buscar o bajar la versión, para mostrarlo en el aviso. */
    var problem by mutableStateOf<String?>(null)
        private set

    /** Si la última búsqueda manual no encontró nada nuevo (para el texto de Ajustes). */
    var upToDate by mutableStateOf(false)
        private set

    /** Busca una versión nueva. Al abrir la app (`manual = false`) respeta el «ahora no». */
    fun check(context: Context, manual: Boolean = false) {
        val app = context.applicationContext
        Store.scope.launch {
            val release = withContext(Dispatchers.IO) { runCatching { latest() }.getOrNull() }
            val skipped = prefs(app).getString("update.skip", null)
            when {
                release == null -> if (manual) problem = "No se pudo revisar si hay versiones nuevas. Revisa tu conexión."
                !newer(release.version, BuildConfig.VERSION_NAME) -> upToDate = manual
                !manual && release.version == skipped -> Unit
                else -> {
                    upToDate = false
                    offered = release
                }
            }
        }
    }

    /** «Ahora no»: no vuelve a avisar de esta versión al abrir (sí desde Ajustes). */
    fun dismiss(context: Context) {
        offered?.let { prefs(context).edit().putString("update.skip", it.version).apply() }
        offered = null
        problem = null
    }

    /** Baja el APK de la versión ofrecida y abre el instalador de Android. */
    fun install(context: Context) {
        val release = offered ?: return
        val app = context.applicationContext
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !app.packageManager.canRequestPackageInstalls()) {
            // Android pide permitir, una sola vez, que TK-Music instale sus actualizaciones.
            val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${app.packageName}"))
            context.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            problem = "Permite «Instalar apps desconocidas» para TK-Music, vuelve y toca Actualizar otra vez."
            return
        }
        problem = null
        progress = 0f
        Store.scope.launch {
            val file = withContext(Dispatchers.IO) {
                runCatching { download(release.url, File(app.cacheDir, "updates").apply { mkdirs() }) }.getOrNull()
            }
            progress = null
            if (file == null) {
                problem = "No se pudo descargar la actualización. Intenta de nuevo."
                return@launch
            }
            val uri = FileProvider.getUriForFile(app, "${app.packageName}.updates", file)
            val intent = Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            runCatching { context.startActivity(intent) }
                .onFailure { problem = "No se pudo abrir el instalador de Android." }
        }
    }

    private fun latest(): Release? {
        val connection = (URL(LATEST).openConnection() as HttpURLConnection).apply {
            setRequestProperty("Accept", "application/vnd.github+json")
            connectTimeout = 10_000
            readTimeout = 10_000
        }
        val json = connection.inputStream.bufferedReader().use { JSONObject(it.readText()) }
        val version = json.getString("tag_name").removePrefix("v")
        val assets = json.getJSONArray("assets")
        val url = (0 until assets.length())
            .map { assets.getJSONObject(it) }
            .firstOrNull { it.getString("name") == APK }
            ?.getString("browser_download_url") ?: return null
        return Release(version, json.optString("body"), url)
    }

    private fun download(url: String, folder: File): File {
        folder.listFiles()?.forEach { it.delete() }
        val target = File(folder, APK)
        val connection = (URL(url).openConnection() as HttpURLConnection).apply {
            instanceFollowRedirects = true
            connectTimeout = 15_000
            readTimeout = 30_000
        }
        val total = connection.contentLengthLong.takeIf { it > 0 }
        connection.inputStream.use { input ->
            target.outputStream().use { output ->
                val buffer = ByteArray(64 * 1024)
                var done = 0L
                while (true) {
                    val read = input.read(buffer)
                    if (read < 0) break
                    output.write(buffer, 0, read)
                    done += read
                    if (total != null) {
                        val fraction = done.toFloat() / total
                        Store.scope.launch { progress = fraction }
                    }
                }
            }
        }
        return target
    }

    /** Si `candidate` (1.2.3) es más nueva que `installed`; una versión de prueba (0.x) siempre pierde. */
    private fun newer(candidate: String, installed: String): Boolean {
        fun parts(version: String) = version.split('.', '-').map { it.toIntOrNull() ?: 0 }
        val a = parts(candidate)
        val b = parts(installed)
        for (i in 0 until maxOf(a.size, b.size)) {
            val x = a.getOrElse(i) { 0 }
            val y = b.getOrElse(i) { 0 }
            if (x != y) return x > y
        }
        return false
    }

    private fun prefs(context: Context) = context.getSharedPreferences("sonora", Context.MODE_PRIVATE)
}

/** El aviso de versión nueva: qué cambió, y Actualizar / Ahora no. */
@Composable
fun UpdateDialog() {
    val release = Updater.offered ?: return
    val context = LocalContext.current
    val downloading = Updater.progress != null
    AlertDialog(
        onDismissRequest = { if (!downloading) Updater.dismiss(context) },
        title = { Text("Nueva versión: ${release.version}") },
        text = {
            Column {
                val notes = release.notes
                    .replace("**", "")
                    .lines()
                    .takeWhile { !it.startsWith("Powered by") }
                    .joinToString("\n")
                    .trim()
                if (notes.isNotEmpty()) {
                    Text(
                        notes,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.heightIn(max = 240.dp).verticalScroll(rememberScrollState()),
                    )
                }
                Updater.progress?.let {
                    Spacer(Modifier.size(16.dp))
                    Text("Descargando…", style = MaterialTheme.typography.labelMedium)
                    Spacer(Modifier.size(8.dp))
                    LinearProgressIndicator(progress = { it }, modifier = Modifier.fillMaxWidth())
                }
                Updater.problem?.let {
                    Spacer(Modifier.size(12.dp))
                    Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
            }
        },
        confirmButton = {
            TextButton(enabled = !downloading, onClick = { Updater.install(context) }) { Text("Actualizar") }
        },
        dismissButton = {
            TextButton(enabled = !downloading, onClick = { Updater.dismiss(context) }) { Text("Ahora no") }
        },
    )
}
