import ActivityKit
import Foundation

/// La Actividad en vivo de TK-Music: la frase que suena en la isla dinámica y en la pantalla
/// bloqueada. La comparten la app (que la actualiza) y la extensión (que la dibuja).
@available(iOS 16.1, *)
struct LiveLyricsAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        var title: String
        var artist: String
        /// La frase que suena; vacía si la canción no tiene letra sincronizada.
        var line: String
        var next: String
        var playing: Bool
        /// El color de la portada, en rojo, verde y azul de 0 a 1.
        var red: Double
        var green: Double
        var blue: Double
    }
}
