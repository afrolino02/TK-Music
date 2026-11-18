//! El estado del bot en cada servidor y lo que hace con la cola: unirse a la voz, agregar
//! canciones, la radio cuando la cola se acaba y el panel de «Sonando ahora».

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use async_trait::async_trait;
use poise::serenity_prelude as serenity;
use serenity::{ChannelId, GuildId, Http, MessageId};
use songbird::events::{Event, EventContext, EventHandler, TrackEvent};
use songbird::tracks::{PlayMode, Track, TrackHandle};
use songbird::{Call, Songbird};
use tokio::sync::Mutex;

use crate::audio::Lazy;
use crate::music::{Music, Song};
use crate::panel;

/// Cada cuánto se actualiza la barra de progreso del panel. Discord limita cuántas veces se
/// puede editar un mensaje, así que no conviene bajarlo mucho.
const TICK: Duration = Duration::from_secs(10);
/// Cuántas canciones agrega la radio cada vez que la cola se acaba.
const RADIO_BATCH: usize = 8;

/// Lo que comparten todos los comandos y eventos.
pub struct Bot {
    pub music: Music,
    pub songbird: Arc<Songbird>,
    pub http: Arc<Http>,
    rooms: Mutex<HashMap<GuildId, Room>>,
    /// El volumen inicial de cada servidor, de 0 a 1.
    default_volume: f32,
}

/// Cómo está el bot en un servidor.
pub struct Room {
    /// Dónde se manda el panel y los avisos.
    pub channel: ChannelId,
    pub panel: Option<MessageId>,
    pub radio: bool,
    pub volume: f32,
    /// La última canción que sonó, para la radio y la letra.
    pub last: Option<Song>,
    /// Se agregaron los eventos de la llamada (se hace una vez por conexión).
    hooked: bool,
    ticker: Option<tokio::task::JoinHandle<()>>,
}

impl Bot {
    pub fn new(music: Music, songbird: Arc<Songbird>, http: Arc<Http>, volume: u8) -> Self {
        Self {
            music,
            songbird,
            http,
            rooms: Mutex::new(HashMap::new()),
            default_volume: f32::from(volume.min(150)) / 100.0,
        }
    }

    /// Hace algo con el estado del servidor, creándolo si hace falta.
    pub async fn room<T>(&self, guild: GuildId, channel: ChannelId, f: impl FnOnce(&mut Room) -> T) -> T {
        let mut rooms = self.rooms.lock().await;
        let room = rooms.entry(guild).or_insert_with(|| Room {
            channel,
            panel: None,
            radio: false,
            volume: self.default_volume,
            last: None,
            hooked: false,
            ticker: None,
        });
        f(room)
    }

    /// El estado del servidor si existe, sin crearlo.
    pub async fn peek<T>(&self, guild: GuildId, f: impl FnOnce(&Room) -> T) -> Option<T> {
        self.rooms.lock().await.get(&guild).map(f)
    }

    /// La llamada del servidor, si el bot está en un canal de voz.
    pub fn call(&self, guild: GuildId) -> Option<Arc<Mutex<Call>>> {
        self.songbird.get(guild)
    }

    /// Entra al canal de voz (o se queda si ya está) y prepara los eventos de la cola.
    pub async fn join(self: &Arc<Self>, guild: GuildId, voice: ChannelId, text: ChannelId) -> Result<Arc<Mutex<Call>>> {
        let call = match self.call(guild) {
            Some(call) if call.lock().await.current_channel().is_some() => call,
            _ => self
                .songbird
                .join(guild, voice)
                .await
                .map_err(|error| anyhow!("no pude entrar al canal de voz: {error}"))?,
        };
        let hook = self
            .room(guild, text, |room| {
                room.channel = text;
                !std::mem::replace(&mut room.hooked, true)
            })
            .await;
        if hook {
            let mut locked = call.lock().await;
            locked.add_global_event(
                Event::Track(TrackEvent::Play),
                Started { bot: self.clone(), guild },
            );
            locked.add_global_event(
                Event::Track(TrackEvent::End),
                Ended { bot: self.clone(), guild },
            );
            locked.add_global_event(
                Event::Track(TrackEvent::Error),
                Failed { bot: self.clone(), guild },
            );
            drop(locked);
            self.start_ticker(guild).await;
        }
        Ok(call)
    }

    /// Agrega canciones al final de la cola.
    pub async fn enqueue(&self, guild: GuildId, songs: Vec<Song>) -> Result<usize> {
        let call = self.call(guild).context("no estoy en ningún canal de voz")?;
        let volume = self.peek(guild, |room| room.volume).await.unwrap_or(self.default_volume);
        let mut call = call.lock().await;
        let count = songs.len();
        for song in songs {
            let data = Arc::new(song.clone());
            let input = Lazy::input(song, self.music.decoder());
            let track = Track::new_with_data(input, data).volume(volume);
            call.enqueue(track).await;
        }
        Ok(count)
    }

    /// Sale del canal de voz y olvida la cola.
    pub async fn leave(&self, guild: GuildId) {
        if let Some(call) = self.call(guild) {
            call.lock().await.queue().stop();
        }
        let _ = self.songbird.remove(guild).await;
        let panel = {
            let mut rooms = self.rooms.lock().await;
            rooms.remove(&guild).map(|room| {
                if let Some(ticker) = room.ticker {
                    ticker.abort();
                }
                (room.channel, room.panel)
            })
        };
        if let Some((channel, Some(message))) = panel {
            panel::retire(&self.http, channel, message, "👋 Me salí del canal de voz.").await;
        }
    }

    /// La canción que suena y su estado, si hay una.
    pub async fn current(&self, guild: GuildId) -> Option<(Song, TrackHandle)> {
        let call = self.call(guild)?;
        let handle = call.lock().await.queue().current()?;
        let song = (*handle.data::<Song>()).clone();
        Some((song, handle))
    }

    /// Lo que viene después de la canción que suena.
    pub async fn upcoming(&self, guild: GuildId) -> Vec<Song> {
        let Some(call) = self.call(guild) else {
            return Vec::new();
        };
        let queue = call.lock().await.queue().current_queue();
        queue
            .iter()
            .skip(1)
            .map(|handle| (*handle.data::<Song>()).clone())
            .collect()
    }

    /// Vuelve a dibujar el panel con la posición de ahora.
    pub async fn refresh_panel(&self, guild: GuildId) {
        let Some((channel, Some(message), radio)) = self.peek(guild, |room| (room.channel, room.panel, room.radio)).await
        else {
            return;
        };
        let Some((song, handle)) = self.current(guild).await else {
            return;
        };
        let Ok(state) = handle.get_info().await else {
            return;
        };
        let waiting = self.upcoming(guild).await.len();
        let paused = matches!(state.playing, PlayMode::Pause);
        let (embed, rows) = panel::now_playing(&song, state.position, paused, radio, waiting);
        let edit = serenity::EditMessage::new().embed(embed).components(rows);
        if let Err(error) = channel.edit_message(&self.http, message, edit).await {
            log::debug!("panel: no se pudo actualizar: {error}");
        }
    }

    /// Manda un panel nuevo para `song` y borra el anterior, para que quede abajo del chat.
    async fn post_panel(&self, guild: GuildId, song: &Song) {
        let Some((channel, old, radio)) = self.peek(guild, |room| (room.channel, room.panel, room.radio)).await else {
            return;
        };
        let waiting = self.upcoming(guild).await.len();
        let (embed, rows) = panel::now_playing(song, Duration::ZERO, false, radio, waiting);
        let sent = channel
            .send_message(&self.http, serenity::CreateMessage::new().embed(embed).components(rows))
            .await;
        if let Some(old) = old {
            let _ = channel.delete_message(&self.http, old).await;
        }
        match sent {
            Ok(message) => {
                self.room(guild, channel, |room| room.panel = Some(message.id)).await;
            }
            Err(error) => log::warn!("panel: no se pudo mandar: {error}"),
        }
    }

    /// Avisa algo en el canal del servidor.
    pub async fn say(&self, guild: GuildId, text: impl Into<String>) {
        let Some(channel) = self.peek(guild, |room| room.channel).await else {
            return;
        };
        let _ = channel.say(&self.http, text.into()).await;
    }

    async fn start_ticker(self: &Arc<Self>, guild: GuildId) {
        let bot = self.clone();
        let ticker = tokio::spawn(async move {
            let mut every = tokio::time::interval(TICK);
            loop {
                every.tick().await;
                bot.refresh_panel(guild).await;
            }
        });
        let channel = self.peek(guild, |room| room.channel).await;
        if let Some(channel) = channel {
            self.room(guild, channel, |room| {
                if let Some(old) = room.ticker.replace(ticker) {
                    old.abort();
                }
            })
            .await;
        }
    }

    /// La cola se quedó vacía: con la radio puesta se agregan parecidas a la última.
    async fn queue_ran_out(&self, guild: GuildId) {
        let Some((radio, last, channel, panel)) = self
            .peek(guild, |room| (room.radio, room.last.clone(), room.channel, room.panel))
            .await
        else {
            return;
        };
        if radio && let Some(last) = last {
            match self.music.radio(&last, RADIO_BATCH).await {
                Ok(songs) if !songs.is_empty() => {
                    if let Err(error) = self.enqueue(guild, songs).await {
                        log::warn!("radio: {error:#}");
                    }
                    return;
                }
                Ok(_) => self.say(guild, "📻 La radio no encontró más canciones parecidas.").await,
                Err(error) => log::warn!("radio: {error:#}"),
            }
        }
        if let Some(panel) = panel {
            panel::retire(&self.http, channel, panel, "✅ Se acabó la cola. Pide más con `/play`.").await;
            self.room(guild, channel, |room| room.panel = None).await;
        }
    }
}

/// Empezó una canción: panel nuevo y volumen del servidor.
struct Started {
    bot: Arc<Bot>,
    guild: GuildId,
}

#[async_trait]
impl EventHandler for Started {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(tracks) = ctx else {
            return None;
        };
        let (_, handle) = tracks.first()?;
        let song = (*handle.data::<Song>()).clone();
        let channel = self.bot.peek(self.guild, |room| room.channel).await?;
        let volume = self
            .bot
            .room(self.guild, channel, |room| {
                room.last = Some(song.clone());
                room.volume
            })
            .await;
        let _ = handle.set_volume(volume);
        self.bot.post_panel(self.guild, &song).await;
        None
    }
}

/// Terminó una canción: si era la última, la radio o el aviso de fin.
struct Ended {
    bot: Arc<Bot>,
    guild: GuildId,
}

#[async_trait]
impl EventHandler for Ended {
    async fn act(&self, _: &EventContext<'_>) -> Option<Event> {
        // la cola quita la canción terminada justo después de este evento
        tokio::time::sleep(Duration::from_millis(400)).await;
        let empty = match self.bot.call(self.guild) {
            Some(call) => call.lock().await.queue().is_empty(),
            None => return None,
        };
        if empty {
            self.bot.queue_ran_out(self.guild).await;
        }
        None
    }
}

/// Una canción no se pudo reproducir.
struct Failed {
    bot: Arc<Bot>,
    guild: GuildId,
}

#[async_trait]
impl EventHandler for Failed {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if let EventContext::Track(tracks) = ctx
            && let Some((_, handle)) = tracks.first()
        {
            let song = handle.data::<Song>();
            self.bot
                .say(self.guild, format!("⚠️ No pude reproducir **{}**; sigo con la siguiente.", song.title))
                .await;
        }
        None
    }
}
