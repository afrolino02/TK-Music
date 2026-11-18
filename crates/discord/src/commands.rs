//! Los comandos de barra: /play, /saltar, /pausa, /seguir, /parar, /cola, /volumen, /radio,
//! /letra, /mezclar, /quitar, /sonando, /salir y /ayuda.

use std::sync::Arc;

use poise::CreateReply;
use poise::serenity_prelude as serenity;
use serenity::{ChannelId, GuildId, UserId};

use crate::bot::Bot;
use crate::config::QUEUE_CHANNEL;
use crate::panel;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Arc<Bot>, Error>;

/// Todos los comandos, para registrarlos.
pub fn all() -> Vec<poise::Command<Arc<Bot>, Error>> {
    vec![
        play(),
        saltar(),
        pausa(),
        seguir(),
        parar(),
        cola(),
        volumen(),
        radio(),
        letra(),
        mezclar(),
        quitar(),
        sonando(),
        salir(),
        ayuda(),
    ]
}

/// Pone una canción o la agrega a la cola: nombre, artista o enlace (canción, playlist o álbum).
#[poise::command(slash_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Nombre, artista o enlace de YouTube / YouTube Music"] busqueda: String,
) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let Some(voice) = voice_channel(ctx.serenity_context(), guild, ctx.author().id) else {
        ctx.send(reply("🎧 Primero entra a un canal de voz.")).await?;
        return Ok(());
    };
    ctx.defer().await?;
    let bot = ctx.data().clone();
    let name = display_name(&ctx).await;
    let text = ctx.channel_id();
    let embed = request(&bot, guild, voice, text, &busqueda, &name).await?;
    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Pasa a la siguiente canción.
#[poise::command(slash_command, guild_only)]
pub async fn saltar(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let text = skip(ctx.data(), guild).await;
    ctx.send(reply(text)).await?;
    Ok(())
}

/// Pausa la música.
#[poise::command(slash_command, guild_only)]
pub async fn pausa(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let text = set_paused(ctx.data(), guild, true).await;
    ctx.send(reply(text)).await?;
    Ok(())
}

/// Sigue con la música pausada.
#[poise::command(slash_command, guild_only)]
pub async fn seguir(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let text = set_paused(ctx.data(), guild, false).await;
    ctx.send(reply(text)).await?;
    Ok(())
}

/// Para la música y vacía la cola (el bot se queda en el canal).
#[poise::command(slash_command, guild_only)]
pub async fn parar(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let text = stop(ctx.data(), guild).await;
    ctx.send(reply(text)).await?;
    Ok(())
}

/// Muestra lo que suena y lo que viene.
#[poise::command(slash_command, guild_only)]
pub async fn cola(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let bot = ctx.data();
    let current = bot.current(guild).await.map(|(song, _)| song);
    let upcoming = bot.upcoming(guild).await;
    ctx.send(CreateReply::default().embed(panel::queue(current.as_ref(), &upcoming))).await?;
    Ok(())
}

/// Cambia el volumen (0 a 150).
#[poise::command(slash_command, guild_only)]
pub async fn volumen(
    ctx: Context<'_>,
    #[description = "De 0 a 150 (100 es normal)"]
    #[min = 0]
    #[max = 150]
    nivel: u8,
) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let bot = ctx.data();
    let volume = f32::from(nivel.min(150)) / 100.0;
    bot.room(guild, ctx.channel_id(), |room| room.volume = volume).await;
    if let Some(call) = bot.call(guild) {
        for handle in call.lock().await.queue().current_queue() {
            let _ = handle.set_volume(volume);
        }
    }
    ctx.send(reply(format!("🔊 Volumen al {nivel} %."))).await?;
    Ok(())
}

/// Activa o desactiva la radio: cuando la cola se acaba, siguen canciones parecidas.
#[poise::command(slash_command, guild_only)]
pub async fn radio(
    ctx: Context<'_>,
    #[description = "Encender o apagar (si no eliges, cambia)"] encender: Option<bool>,
) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let on = toggle_radio(ctx.data(), guild, ctx.channel_id(), encender).await;
    let text = match on {
        true => "📻 Radio encendida: cuando la cola se acabe, siguen canciones parecidas.",
        false => "📻 Radio apagada.",
    };
    ctx.send(reply(text)).await?;
    Ok(())
}

/// La letra de lo que suena.
#[poise::command(slash_command, guild_only)]
pub async fn letra(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    ctx.defer().await?;
    let embeds = lyrics(ctx.data(), guild).await;
    let mut message = CreateReply::default();
    for embed in embeds {
        message = message.embed(embed);
    }
    ctx.send(message).await?;
    Ok(())
}

/// Revuelve el orden de lo que viene en la cola.
#[poise::command(slash_command, guild_only)]
pub async fn mezclar(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let Some(call) = ctx.data().call(guild) else {
        ctx.send(reply("No estoy en ningún canal de voz.")).await?;
        return Ok(());
    };
    let count = call.lock().await.queue().modify_queue(|queue| {
        if queue.len() < 3 {
            return queue.len().saturating_sub(1);
        }
        // la primera es la que suena: se queda en su sitio
        let rest = queue.make_contiguous();
        fastrand::shuffle(&mut rest[1..]);
        rest.len() - 1
    });
    ctx.send(reply(format!("🔀 Mezclé {count} canciones de la cola."))).await?;
    Ok(())
}

/// Quita una canción de la cola por su número (el que sale en /cola).
#[poise::command(slash_command, guild_only)]
pub async fn quitar(
    ctx: Context<'_>,
    #[description = "El número que sale en /cola"]
    #[min = 1]
    numero: usize,
) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let Some(call) = ctx.data().call(guild) else {
        ctx.send(reply("No estoy en ningún canal de voz.")).await?;
        return Ok(());
    };
    let removed = call.lock().await.queue().dequeue(numero);
    let text = match removed {
        Some(queued) => {
            let song = queued.handle().data::<crate::music::Song>();
            let _ = queued.stop();
            format!("🗑️ Quité **{}** de la cola.", song.title)
        }
        None => format!("No hay ninguna canción con el número {numero} en la cola."),
    };
    ctx.send(reply(text)).await?;
    Ok(())
}

/// Vuelve a mandar el panel de lo que suena al final del chat.
#[poise::command(slash_command, guild_only)]
pub async fn sonando(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    let bot = ctx.data();
    let Some((song, handle)) = bot.current(guild).await else {
        ctx.send(reply("No suena nada.")).await?;
        return Ok(());
    };
    let state = handle.get_info().await?;
    let radio = bot.peek(guild, |room| room.radio).await.unwrap_or(false);
    let waiting = bot.upcoming(guild).await.len();
    let paused = matches!(state.playing, songbird::tracks::PlayMode::Pause);
    let (embed, rows) = panel::now_playing(&song, state.position, paused, radio, waiting);
    let sent = ctx.send(CreateReply::default().embed(embed).components(rows)).await?;
    let message = sent.message().await?.id;
    bot.room(guild, ctx.channel_id(), |room| {
        room.channel = ctx.channel_id();
        room.panel = Some(message);
    })
    .await;
    Ok(())
}

/// Saca al bot del canal de voz.
#[poise::command(slash_command, guild_only)]
pub async fn salir(ctx: Context<'_>) -> Result<(), Error> {
    let guild = ctx.guild_id().ok_or("solo funciona en un servidor")?;
    ctx.data().leave(guild).await;
    ctx.send(reply("👋 Listo, me salí.")).await?;
    Ok(())
}

/// Cómo se usa el bot.
#[poise::command(slash_command)]
pub async fn ayuda(ctx: Context<'_>) -> Result<(), Error> {
    let text = format!(
        "**TK-Music** pone música de YouTube Music en tu canal de voz.\n\n\
         `/play` canción, artista o enlace · `/saltar` · `/pausa` · `/seguir` · `/parar`\n\
         `/cola` · `/quitar` · `/mezclar` · `/volumen` · `/radio` · `/letra` · `/sonando` · `/salir`\n\n\
         📱 **Desde el teléfono:** escribe el nombre o pega un enlace en el canal **#{QUEUE_CHANNEL}** \
         (o usa «Enviar al bot de Discord» en la app TK-Music) y se agrega a la cola."
    );
    ctx.send(reply(text)).await?;
    Ok(())
}

// ── lo que comparten los comandos, los botones y el canal de la cola ─────────────────────────

/// Busca, entra al canal de voz si hace falta y agrega a la cola. Devuelve el aviso.
pub async fn request(
    bot: &Arc<Bot>,
    guild: GuildId,
    voice: ChannelId,
    text: ChannelId,
    query: &str,
    requested_by: &str,
) -> Result<serenity::CreateEmbed, Error> {
    let started = std::time::Instant::now();
    let songs = match bot.music.find(query, requested_by).await {
        Ok(songs) => songs,
        Err(error) => return Ok(panel::note(format!("🔎 {error}"))),
    };
    log::info!("buscar «{query}»: {:.1} s", started.elapsed().as_secs_f32());
    let playing_now = bot.current(guild).await.is_none();
    // si no suena nada, el audio de la primera se baja mientras el bot entra al canal
    let first = playing_now.then(|| crate::audio::prefetch(&songs[0], bot.music.decoder()));
    // contestar ya, y entrar y encolar por detrás: Discord deja de mostrar «pensando…»
    let bot = bot.clone();
    let queued = songs.clone();
    tokio::spawn(async move {
        let result = async {
            bot.join(guild, voice, text).await?;
            bot.enqueue(guild, queued, first).await
        }
        .await;
        if let Err(error) = result {
            bot.say(guild, format!("⚠️ {error}")).await;
        }
    });
    Ok(panel::added(&songs, playing_now))
}

pub async fn skip(bot: &Bot, guild: GuildId) -> String {
    match bot.call(guild) {
        Some(call) => match call.lock().await.queue().skip() {
            Ok(()) => "⏭️ Siguiente.".to_owned(),
            Err(_) => "No hay nada sonando.".to_owned(),
        },
        None => "No estoy en ningún canal de voz.".to_owned(),
    }
}

pub async fn set_paused(bot: &Bot, guild: GuildId, paused: bool) -> String {
    let Some(call) = bot.call(guild) else {
        return "No estoy en ningún canal de voz.".to_owned();
    };
    let queue = call.lock().await.queue().clone();
    let done = match paused {
        true => queue.pause(),
        false => queue.resume(),
    };
    bot.refresh_panel(guild).await;
    match (done, paused) {
        (Err(_), _) => "No hay nada sonando.".to_owned(),
        (Ok(()), true) => "⏸️ En pausa.".to_owned(),
        (Ok(()), false) => "▶️ Sigue la música.".to_owned(),
    }
}

pub async fn stop(bot: &Bot, guild: GuildId) -> String {
    match bot.call(guild) {
        Some(call) => {
            call.lock().await.queue().stop();
            "⏹️ Paré la música y vacié la cola.".to_owned()
        }
        None => "No estoy en ningún canal de voz.".to_owned(),
    }
}

pub async fn toggle_radio(bot: &Bot, guild: GuildId, channel: ChannelId, on: Option<bool>) -> bool {
    let now = bot
        .room(guild, channel, |room| {
            room.radio = on.unwrap_or(!room.radio);
            room.radio
        })
        .await;
    bot.refresh_panel(guild).await;
    now
}

pub async fn lyrics(bot: &Bot, guild: GuildId) -> Vec<serenity::CreateEmbed> {
    let Some((song, _)) = bot.current(guild).await else {
        return vec![panel::note("No suena nada.")];
    };
    match bot.music.lyrics(&song).await {
        Some(text) => panel::lyrics(&song, &text),
        None => vec![panel::note(format!("No encontré la letra de **{}**.", song.title))],
    }
}

/// El canal de voz donde está alguien, según la caché de Discord.
pub fn voice_channel(ctx: &serenity::Context, guild: GuildId, user: UserId) -> Option<ChannelId> {
    let guild = ctx.cache.guild(guild)?;
    guild.voice_states.get(&user).and_then(|state| state.channel_id)
}

async fn display_name(ctx: &Context<'_>) -> String {
    match ctx.author_member().await {
        Some(member) => member.display_name().to_owned(),
        None => ctx.author().name.clone(),
    }
}

fn reply(text: impl Into<String>) -> CreateReply {
    CreateReply::default().embed(panel::note(text))
}
