# Changelog de TK-Music

Aquí están los cambios de cada versión de TK-Music, de la más nueva a la más vieja. Las
descargas de cada una están en [Releases](https://github.com/rprofe358/TK-Music/releases).

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/): **Nuevo** para
lo que se puede hacer ahora, **Cambios** para lo que funciona distinto y **Arreglos** para los
fallos corregidos. Lo que dice _(prueba)_ todavía no llega por la actualización automática.

TK-Music está hecho sobre [Sonora](https://github.com/sonorahq/sonora), de Pavel Olizko. Los
cambios de Sonora están en [CHANGELOG-SONORA.md](CHANGELOG-SONORA.md).

## [Sin publicar] — v1.0.13 _(prueba)_

### Nuevo

- El inicio sale al instante con lo último que viste y se actualiza solo por detrás. Mientras carga por primera vez se ven siluetas de las tarjetas en vez del círculo girando.
- Pantalla «Tus canciones sin conexión» en la Biblioteca, con buscador, para lo guardado dentro de la app.
- Sin internet, Buscar busca en la música descargada del teléfono y en lo guardado para sin conexión.
- Acceso a «Música descargada» en la Biblioteca aunque todavía no esté activada.

### Cambios

- El selector de servicios (YouTube Music, Spotify, Descargadas) ahora son pestañas con icono.
- La app siempre abre en YouTube Music (o en el último servicio que elegiste), no en Descargadas.
- «Descargar para sin conexión» pasa a llamarse «Guardar para sin conexión», para no confundirlo con la música descargada en el teléfono.
- El inicio pide sus partes (estantes, me gusta y playlists) a la vez, así carga más rápido.

### Arreglos

- Música descargada (Android): siempre se revisan las carpetas Descargas y Música, también en la tarjeta SD, y entra el audio que Android no marca como música. Antes faltaban canciones o no salía ninguna.
- Las playlists que ya tienes salían vacías y no se podían crear, porque la app abría en la pestaña Descargadas.
- Lo guardado para sin conexión ahora suena también en modo avión, sin entrar a la cuenta.
- Sin internet ya no aparece un aviso de error de la cuenta al abrir la app.

## [1.0.11] — 2026-11-16

### Nuevo

- Guardar canciones para escucharlas sin conexión, desde el menú de cada canción y desde el reproductor.
- Botón para guardar una playlist o un álbum completo, junto a Reproducir y Aleatorio.
- Lo guardado sale primero en el modo sin conexión y no se borra solo.

## [1.0.10] — 2026-11-16

### Arreglos

- Android: la app se cerraba al abrir en la 1.0.9.

## [1.0.9] — 2026-11-16

### Nuevo

- Letras con pronunciación para canciones en japonés, coreano, chino, ruso, griego o árabe.
- La pantalla no se apaga mientras ves las letras.
- Modo sin conexión: sin internet, Inicio muestra lo que ya está guardado en el teléfono.
- «Colores de la portada»: la app toma el color de lo que suena.
- «Fondo ambiental»: los colores de la portada se mueven detrás del reproductor.
- Visualizador con barras, onda o los dos.

## [1.0.8] — 2026-11-16

### Nuevo

- PC: animación de TK-Music al abrir la app.

### Arreglos

- PC: TK-Music ya no se confunde con Sonora cuando las dos están instaladas.

## [1.0.7] — 2026-11-15

### Nuevo

- TK-Music para PC (Windows), con instalador `TK-Music-Setup.exe`.
- Android: aviso de versión nueva y actualización desde la misma app.
- PC: se actualiza sola.

## [1.0.6] — 2026-11-15

### Arreglos

- Android: la app se cerraba al abrirla desde la 1.0.1.

## [1.0.5] — 2026-11-15

### Arreglos

- Android: el error «el paquete no es válido» al instalar encima de una versión anterior.

## [1.0.4] — 2026-11-14

### Nuevo

- TK-Music aparece como fuente de música en carros con Android Automotive.

## [1.0.3] — 2026-11-14

### Nuevo

- `TK-Music-emulador.apk` para probar la app en el emulador de Android Studio.

## [1.0.2] — 2026-11-13

### Arreglos

- Las canciones que YouTube Music da como video muestran la portada del álbum en vez de la foto del artista, también en las canciones populares de cada artista.

## [1.0.1] — 2026-11-13

### Nuevo

- Android: se puede quitar cualquier carpeta de la música del teléfono y volver a mostrarla.

### Arreglos

- Android: los audios y notas de voz de WhatsApp ya no salen como canciones.
- Las canciones de YouTube Music muestran la portada del álbum en vez de la foto del artista.

## [1.0.0] — 2026-11-13

### Nuevo

- Primera versión de TK-Music en su propio repositorio: Android, Android TV, Android Auto, iPhone y iPad.

[Sin publicar]: https://github.com/rprofe358/TK-Music/compare/v1.0.11...main
[1.0.11]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.11
[1.0.10]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.10
[1.0.9]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.9
[1.0.8]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.8
[1.0.7]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.7
[1.0.6]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.6
[1.0.5]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.5
[1.0.4]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.4
[1.0.3]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.3
[1.0.2]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.2
[1.0.1]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.1
[1.0.0]: https://github.com/rprofe358/TK-Music/releases/tag/v1.0.0
