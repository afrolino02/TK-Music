<div align="center">

<img src="./ios/SonoraMobile/Assets.xcassets/AppIcon.appiconset/icon-1024.png" alt="" width="96" height="96">

<h1>TK-Music</h1>

**Tu música en iPhone, iPad, Android, Android TV, Android Auto, Windows y Mac.**

<sub>Powered by <a href="https://github.com/sonorahq/sonora">Sonora</a></sub>

[![Android](https://img.shields.io/github/actions/workflow/status/rprofe358/TK-Music/android.yml?branch=main&style=flat-square&label=android)](https://github.com/rprofe358/TK-Music/actions/workflows/android.yml)
[![iOS](https://img.shields.io/github/actions/workflow/status/rprofe358/TK-Music/ios.yml?branch=main&style=flat-square&label=ios)](https://github.com/rprofe358/TK-Music/actions/workflows/ios.yml)
[![Licencia](https://img.shields.io/badge/licencia-GPL--3.0--or--later-blue?style=flat-square)](./COPYING)

</div>

## Descargas

| Plataforma | Archivo | Cómo instalar |
|---|---|---|
| Android, Android TV y Android Auto | [**TK-Music.apk**](https://github.com/rprofe358/TK-Music/releases/latest/download/TK-Music.apk) | Abre el APK en el dispositivo y permite instalar apps de origen desconocido. |
| Windows (PC) | [**TK-Music-Setup.exe**](https://github.com/rprofe358/TK-Music/releases/latest/download/TK-Music-Setup.exe) | Ejecuta el instalador. Si Windows avisa, toca «Más información» → «Ejecutar de todas formas». Se actualiza sola. |
| Mac | [**TK-Music-macos.dmg**](https://github.com/rprofe358/TK-Music/releases/latest/download/TK-Music-macos.dmg) | Abre el .dmg y arrastra TK-Music a Aplicaciones. La primera vez macOS avisa que no puede verificarla: Ajustes del Sistema → Privacidad y seguridad → «Abrir igualmente». Se actualiza sola. |
| Bot de Discord (PC) | [**TK-Music-Bot.exe**](https://github.com/rprofe358/TK-Music/releases/latest/download/TK-Music-Bot.exe) | Pone la música en tu canal de voz. Guía paso a paso: [docs/DISCORD.md](docs/DISCORD.md). |
| iPhone y iPad | [**TK-Music.ipa**](https://github.com/rprofe358/TK-Music/releases/latest/download/TK-Music.ipa) | Instálalo con Sideloadly o AltStore (con una cuenta gratuita se renueva cada 7 días). |

Todas las versiones están en [Releases](https://github.com/rprofe358/TK-Music/releases).

## Qué trae

- Spotify, YouTube Music y tu música descargada en una sola app.
- Inicio con saludo, recomendación destacada, mixes diarios según lo que escuchas y géneros y estados de ánimo para explorar.
- Búsqueda predictiva; al tocar una canción suenan otras parecidas después.
- Reproductor con fondo desenfocado, letras sincronizadas tipo karaoke, visualizador, modo horizontal y aleatorio por género.
- Cola editable: arrastrar para reordenar, quitar una canción, vaciarla o agregar canciones.
- Temas de color personalizables, claro u oscuro.
- Diseño para tablets, Android TV (control remoto) y Android Auto.

## Publicar una versión nueva

```sh
git tag v1.1.0
git push origin v1.1.0
```

GitHub Actions compila el APK, el IPA, el instalador de Windows y el .dmg de Mac y los sube a una nueva versión
en Releases. La app de Android y la de PC avisan solas de la versión nueva. Cada envío a
`main` también compila y deja los archivos en la pestaña Actions.

## Créditos

TK-Music es una versión móvil personal construida sobre **[Sonora](https://github.com/sonorahq/sonora)**,
el cliente de música nativo creado por **Pavel Olizko** y la comunidad de
[sonorahq](https://github.com/sonorahq). El motor de reproducción, los proveedores de música y la
mayor parte del código vienen de Sonora. Gracias por hacerlo libre.

El README original de Sonora está en [README-SONORA.md](./README-SONORA.md) y las licencias de
terceros en [THIRD-PARTY.md](./THIRD-PARTY.md).

## Licencia

Igual que Sonora, TK-Music se distribuye bajo la licencia
[GNU GPL v3.0 o posterior](./COPYING). Puedes usar, estudiar, modificar y compartir el código,
siempre que las versiones que distribuyas mantengan la misma licencia y publiquen su código.
