# TK-Music para Discord

`TK-Music-Bot.exe` es un bot de música para tu servidor de Discord. Corre en tu PC: mientras la
ventana esté abierta, el bot pone en tu canal de voz la música de YouTube Music, con el mismo
motor de TK-Music.

## 1. Crear el bot en Discord (una sola vez)

1. Entra a <https://discord.com/developers/applications> con tu cuenta de Discord y toca
   **New Application**. Ponle un nombre, por ejemplo `TK-Music`, y acepta.
2. En el menú de la izquierda entra a **Bot**:
   - Toca **Reset Token** y copia el token. **No se lo pases a nadie**: con él cualquiera
     controla el bot.
   - Baja hasta **Privileged Gateway Intents** y activa **Message Content Intent** (sirve para
     el canal `#tk-music`). Guarda los cambios.
   - Si quieres que solo tú puedas agregarlo a servidores, desactiva **Public Bot**.
3. Entra a **General Information**, copia el **Application ID** y abre este enlace cambiando
   `TU_ID` por ese número:

   ```
   https://discord.com/oauth2/authorize?client_id=TU_ID&scope=bot+applications.commands&permissions=3229696
   ```

   Elige tu servidor y acepta. El bot entra con permisos para ver canales, escribir, mandar
   embeds, conectarse y hablar en voz.

## 2. Abrir el bot en el PC

1. Descarga [**TK-Music-Bot.exe**](https://github.com/rprofe358/TK-Music/releases/latest/download/TK-Music-Bot.exe).
   Si Windows avisa, toca **Más información → Ejecutar de todas formas**.
2. La primera vez te pide el token: pégalo (clic derecho en la ventana) y presiona Enter. Queda
   guardado en `%APPDATA%\TK-Music-Bot\config.json`.
3. Cuando diga **«✅ … está conectado»**, ya está. Deja la ventana abierta mientras lo usen.

Si en este PC iniciaste sesión en YouTube Music dentro de **TK-Music para PC**, el bot usa esa
misma cuenta (tus playlists y, con Premium, mejor calidad). Si no, entra como invitado.

Para cambiar el token: abre el bot con `TK-Music-Bot.exe --olvidar-token` y vuelve a abrirlo.

## 3. Usarlo

Entra a un canal de voz y escribe en cualquier canal de texto:

| Comando | Qué hace |
| --- | --- |
| `/play` | Pone una canción o la agrega a la cola: nombre, artista o enlace (canción, playlist o álbum de YouTube). |
| `/saltar` · `/pausa` · `/seguir` · `/parar` | Siguiente, pausa, seguir, parar y vaciar la cola. |
| `/cola` · `/quitar` · `/mezclar` | Ver la cola, quitar una por su número, revolverla. |
| `/volumen` | De 0 a 150 (100 es normal). |
| `/radio` | Cuando la cola se acaba, siguen canciones parecidas. |
| `/letra` | La letra de lo que suena. |
| `/sonando` | Vuelve a mandar el panel abajo del chat. |
| `/salir` | Saca al bot del canal de voz. |

El panel de **Sonando ahora** muestra la portada, una barra de progreso que se mueve sola y
botones para pausar, saltar, parar, ver la cola, la radio y la letra.

## 4. Agregar canciones desde el teléfono

- **Desde Discord:** usa `/play` en la app de Discord del teléfono, o crea un canal de texto
  llamado **`tk-music`**: todo lo que se escriba ahí (un nombre o un enlace) se agrega a la cola.
- **Desde la app TK-Music:**
  1. En Discord, entra a los ajustes del canal `#tk-music` → **Integraciones** → **Webhooks** →
     **Nuevo webhook** → **Copiar URL del webhook**.
  2. En TK-Music (Android o iPhone): **Ajustes → Bot de Discord**, pega la URL y toca **Guardar**.
  3. En el menú ⋮ de cualquier canción de YouTube Music sale **«Enviar al bot de Discord»**.

  Funciona aunque no estés en tu casa: la canción viaja por Discord. El bot tiene que estar ya
  en un canal de voz (pon algo con `/play` primero).

## Problemas comunes

- **No aparecen los comandos `/`:** espera un minuto y reinicia Discord. Revisa que el enlace de
  invitación tenga `applications.commands`.
- **El canal `#tk-music` no hace nada:** falta activar **Message Content Intent** (paso 1.2); al
  abrir, el bot avisa si Discord no le da ese permiso.
- **«No pude reproducir…»:** YouTube a veces niega una canción; el bot sigue con la siguiente.
- Spotify no se puede poner en el bot: sus reglas no permiten transmitir su audio.
