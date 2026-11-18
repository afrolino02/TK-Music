import SwiftUI

/// El puente con el bot de Discord de TK-Music: la canción se manda, por un webhook, al canal
/// #tk-music del servidor, y el bot la agrega a su cola. Funciona desde cualquier lado.
extension Store {
    /// Solo las canciones de YouTube Music tienen un enlace que el bot sepa abrir.
    func canSendToDiscord(_ slug: String) -> Bool {
        !discordWebhook.isEmpty && slug == "youtube"
    }

    /// Manda la canción al canal del bot.
    func sendToDiscord(_ track: TrackItem) {
        guard let url = URL(string: discordWebhook) else {
            problem = "El webhook del bot de Discord no es válido. Revísalo en Ajustes."
            return
        }
        let sender = userName.isEmpty ? "iPhone" : userName
        let body: [String: String] = [
            "content": "https://music.youtube.com/watch?v=\(track.id)",
            "username": "TK-Music · \(sender)",
        ]
        var request = URLRequest(url: url, timeoutInterval: 10)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        Task {
            let sent: Bool
            do {
                let (_, response) = try await URLSession.shared.data(for: request)
                sent = ((response as? HTTPURLResponse)?.statusCode ?? 0) / 100 == 2
            } catch {
                sent = false
            }
            await MainActor.run {
                problem = sent
                    ? "Enviada al bot de Discord: \(track.title)"
                    : "No se pudo enviar al bot. Revisa el webhook en Ajustes y tu conexión."
            }
        }
    }
}

/// «Enviar al bot de Discord», en el menú de una canción.
struct DiscordButton: View {
    @EnvironmentObject private var store: Store
    let track: TrackItem
    let slug: String

    var body: some View {
        if store.canSendToDiscord(slug) {
            Button {
                store.sendToDiscord(track)
            } label: {
                Label("Enviar al bot de Discord", systemImage: "paperplane")
            }
        }
    }
}

/// La sección de Ajustes donde se pega el webhook del canal #tk-music.
struct DiscordSection: View {
    @EnvironmentObject private var store: Store
    @State private var draft = ""

    var body: some View {
        Section {
            TextField("https://discord.com/api/webhooks/…", text: $draft)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .onAppear { draft = store.discordWebhook }
            Button("Guardar") {
                let url = draft.trimmingCharacters(in: .whitespacesAndNewlines)
                if url.isEmpty || url.hasPrefix("https://discord.com/api/webhooks/")
                    || url.hasPrefix("https://discordapp.com/api/webhooks/") {
                    store.discordWebhook = url
                    store.problem = url.isEmpty ? "Bot de Discord desconectado." : "Webhook guardado."
                } else {
                    store.problem = "Eso no parece un webhook de Discord: empieza por https://discord.com/api/webhooks/"
                }
            }
        } header: {
            Text("Bot de Discord")
        } footer: {
            Text("En Discord: ajustes del canal #tk-music → Integraciones → Webhooks → Nuevo webhook → Copiar URL. Luego, en el menú de cada canción sale «Enviar al bot de Discord».")
        }
    }
}
