//! TK-Music para Discord: un bot que pone la música de YouTube Music en un canal de voz, con el
//! motor de TK-Music. Corre en el PC de quien lo abre.

mod audio;
mod bot;
mod commands;
mod config;
mod music;
mod panel;

use std::sync::Arc;

use poise::serenity_prelude as serenity;
use serenity::{
    ComponentInteraction, CreateInteractionResponse, CreateInteractionResponseFollowup, FullEvent,
    GatewayIntents, Interaction, Message,
};
use songbird::SerenityInit as _;

use crate::bot::Bot;
use crate::commands::Error;
use crate::config::{Config, QUEUE_CHANNEL};
use crate::panel::button;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,tk_music_bot=info"))
        .init();
    println!("TK-Music para Discord · Powered by Sonora");
    if std::env::args().any(|arg| arg == "--olvidar-token") {
        match Config::forget_token() {
            Ok(()) => println!("Token borrado. Abre el bot otra vez para poner uno nuevo."),
            Err(error) => println!("No se pudo borrar el token: {error:#}"),
        }
        return;
    }
    if let Err(error) = run().await {
        eprintln!();
        eprintln!("❌ {error}");
        eprintln!();
        eprintln!("Presiona Enter para cerrar.");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

async fn run() -> Result<(), Error> {
    let config = Config::load()?;
    println!("Conectando con YouTube Music…");
    let music = music::Music::connect().await?;
    println!("YouTube Music listo ({}).", music.account());
    music.warm_up();

    let full = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    match start(&config, music, full).await {
        Err(error) if disallowed(&error) => {
            println!();
            println!("⚠️  Discord no le da permiso al bot de leer mensajes, así que el canal #{QUEUE_CHANNEL}");
            println!("   no va a funcionar. Para activarlo: portal de desarrolladores → tu app → Bot →");
            println!("   «Message Content Intent» → activar, y vuelve a abrir el bot.");
            println!("   Los comandos con / funcionan igual.");
            println!();
            let music = music::Music::connect().await?;
            start(&config, music, GatewayIntents::non_privileged()).await
        }
        result => result,
    }
}

async fn start(config: &Config, music: music::Music, intents: GatewayIntents) -> Result<(), Error> {
    let volume = config.volume;
    let manager = songbird::Songbird::serenity();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            event_handler: |ctx, event, _framework, bot| Box::pin(on_event(ctx, event, bot)),
            on_error: |error| Box::pin(on_error(error)),
            ..Default::default()
        })
        .setup({
            let manager = manager.clone();
            move |ctx, ready, framework| {
                Box::pin(async move {
                    for guild in &ready.guilds {
                        if let Err(error) =
                            poise::builtins::register_in_guild(ctx, &framework.options().commands, guild.id).await
                        {
                            log::warn!("no se pudieron registrar los comandos en {}: {error}", guild.id);
                        }
                    }
                    println!();
                    println!("✅ {} está conectado en {} servidor(es).", ready.user.name, ready.guilds.len());
                    println!("   Entra a un canal de voz y escribe /play en Discord. Deja esta ventana abierta.");
                    Ok(Arc::new(Bot::new(music, manager, ctx.http.clone(), volume)))
                })
            }
        })
        .build();
    let mut client = serenity::ClientBuilder::new(config.token.trim(), intents)
        .framework(framework)
        .register_songbird_with(manager)
        .await?;
    client.start().await?;
    Ok(())
}

/// Si el error es que Discord no deja usar el permiso de leer mensajes.
fn disallowed(error: &Error) -> bool {
    let text = format!("{error:?}");
    text.contains("DisallowedGatewayIntents") || text.contains("4014")
}

async fn on_event(ctx: &serenity::Context, event: &FullEvent, bot: &Arc<Bot>) -> Result<(), Error> {
    match event {
        FullEvent::InteractionCreate {
            interaction: Interaction::Component(press),
        } => on_button(ctx, press, bot).await,
        FullEvent::Message { new_message } => on_message(ctx, new_message, bot).await,
        FullEvent::GuildCreate { guild, is_new: Some(true) } => {
            // un servidor nuevo: se le registran los comandos de una vez
            let commands = commands::all();
            let commands = poise::builtins::create_application_commands(&commands);
            guild.id.set_commands(&ctx.http, commands).await?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Los botones del panel.
async fn on_button(ctx: &serenity::Context, press: &ComponentInteraction, bot: &Arc<Bot>) -> Result<(), Error> {
    let Some(guild) = press.guild_id else {
        return Ok(());
    };
    let id = press.data.custom_id.as_str();
    if !id.starts_with("tk:") {
        return Ok(());
    }
    // contestar enseguida: Discord da 3 segundos
    press.create_response(&ctx.http, CreateInteractionResponse::Acknowledge).await?;
    let notice = match id {
        button::TOGGLE => {
            let paused = match bot.current(guild).await {
                Some((_, handle)) => handle
                    .get_info()
                    .await
                    .map(|state| matches!(state.playing, songbird::tracks::PlayMode::Pause))
                    .unwrap_or(false),
                None => false,
            };
            commands::set_paused(bot, guild, !paused).await;
            None
        }
        button::SKIP => {
            commands::skip(bot, guild).await;
            None
        }
        button::STOP => Some(commands::stop(bot, guild).await),
        button::RADIO => {
            let on = commands::toggle_radio(bot, guild, press.channel_id, None).await;
            Some(match on {
                true => "📻 Radio encendida.".to_owned(),
                false => "📻 Radio apagada.".to_owned(),
            })
        }
        button::LYRICS => {
            let embeds = commands::lyrics(bot, guild).await;
            let followup = CreateInteractionResponseFollowup::new().embeds(embeds).ephemeral(true);
            press.create_followup(&ctx.http, followup).await?;
            None
        }
        button::QUEUE => {
            let current = bot.current(guild).await.map(|(song, _)| song);
            let upcoming = bot.upcoming(guild).await;
            let followup = CreateInteractionResponseFollowup::new()
                .embed(panel::queue(current.as_ref(), &upcoming))
                .ephemeral(true);
            press.create_followup(&ctx.http, followup).await?;
            None
        }
        _ => None,
    };
    if let Some(text) = notice {
        let who = press.user.display_name();
        let followup = CreateInteractionResponseFollowup::new().embed(panel::note(format!("{text} ({who})")));
        press.create_followup(&ctx.http, followup).await?;
    }
    Ok(())
}

/// Lo que llega al canal #tk-music: cada mensaje es una canción para la cola. Así se agregan
/// desde el teléfono, escribiendo ahí o con el botón de la app TK-Music (que usa un webhook).
async fn on_message(ctx: &serenity::Context, message: &Message, bot: &Arc<Bot>) -> Result<(), Error> {
    let Some(guild) = message.guild_id else {
        return Ok(());
    };
    if message.author.id == ctx.cache.current_user().id || message.content.trim().is_empty() {
        return Ok(());
    }
    let named = match message.channel_id.to_channel(&ctx.http).await {
        Ok(serenity::Channel::Guild(channel)) => channel.name == QUEUE_CHANNEL,
        _ => false,
    };
    if !named {
        return Ok(());
    }
    // una persona: el canal de voz donde está; un webhook: donde ya esté sonando el bot
    let voice = match message.webhook_id {
        None => commands::voice_channel(ctx, guild, message.author.id),
        Some(_) => None,
    }
    .or_else(|| {
        bot.call(guild).and_then(|call| {
            call.try_lock()
                .ok()
                .and_then(|call| call.current_channel())
                .map(|channel| serenity::ChannelId::new(channel.0.get()))
        })
    });
    let Some(voice) = voice else {
        let _ = message
            .reply(&ctx.http, "🎧 Entra a un canal de voz (o pon algo con /play) y vuelve a mandarla.")
            .await;
        return Ok(());
    };
    let name = message
        .author_nick(&ctx.http)
        .await
        .unwrap_or_else(|| message.author.display_name().to_owned());
    let embed = commands::request(bot, guild, voice, message.channel_id, &message.content, &name).await?;
    let _ = message
        .channel_id
        .send_message(&ctx.http, serenity::CreateMessage::new().embed(embed).reference_message(message))
        .await;
    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Arc<Bot>, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            log::warn!("/{}: {error}", ctx.command().name);
            let reply = poise::CreateReply::default()
                .embed(panel::note(format!("⚠️ {error}")))
                .ephemeral(true);
            let _ = ctx.send(reply).await;
        }
        other => {
            if let Err(error) = poise::builtins::on_error(other).await {
                log::warn!("error manejando un error: {error}");
            }
        }
    }
}
