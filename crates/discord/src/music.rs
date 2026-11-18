//! Lo que el bot le pide a YouTube Music: buscar, abrir enlaces, la radio de una canción, la
//! letra y el audio. Usa el motor de TK-Music y, si en este PC ya se inició sesión en TK-Music,
//! la misma cuenta.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use futures::future::join_all;
use music::engine::Decode;
use music::link::Linked;
use music::youtube::YouTubeProvider;
use music::{
    Lyrics, LyricsHit, LyricsProvider, LyricsQuery, MusicApi, MusicProvider as _, ProviderSession,
    SignIn, Track, TrackKey,
};

/// Cuánto se espera a cada servicio de letras.
const LYRICS_PATIENCE: Duration = Duration::from_secs(6);
/// Cuántas canciones se agregan como mucho de una playlist o álbum.
const LIST_LIMIT: usize = 100;

/// Una canción en la cola del bot.
#[derive(Clone, Debug)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artists: String,
    pub album: String,
    pub cover: Option<String>,
    pub duration: Duration,
    /// Quién la pidió, para mostrarlo en el panel.
    pub requested_by: String,
}

impl Song {
    fn from_track(track: &Track, requested_by: &str) -> Option<Self> {
        if !track.playable {
            return None;
        }
        Some(Self {
            id: track.id.clone()?,
            title: track.name.clone(),
            artists: track.artists.clone(),
            album: track.album.clone(),
            cover: track.cover.as_deref().map(sized),
            duration: track.duration,
            requested_by: requested_by.to_owned(),
        })
    }

    /// El enlace de la canción en YouTube Music.
    pub fn url(&self) -> String {
        format!("https://music.youtube.com/watch?v={}", self.id)
    }
}

/// YouTube Music, con la sesión de TK-Music si la hay.
pub struct Music {
    provider: YouTubeProvider,
    session: ProviderSession,
    decoder: Arc<dyn Decode>,
    lyrics: Vec<Arc<dyn LyricsProvider>>,
}

impl Music {
    /// Abre YouTube Music: con la cuenta guardada por TK-Music en este PC, o como invitado.
    pub async fn connect() -> Result<Self> {
        let provider = YouTubeProvider::new();
        let session = match provider.restore().await {
            Ok(Some(session)) => session,
            Ok(None) => guest(&provider).await?,
            Err(error) => {
                log::warn!("youtube: no se pudo recuperar la cuenta, sigo como invitado: {error:#}");
                guest(&provider).await?
            }
        };
        let decoder = session
            .playback
            .decoder()
            .context("YouTube Music no puede entregar el audio por separado")?;
        Ok(Self {
            provider,
            session,
            decoder,
            lyrics: vec![
                Arc::new(music::betterlyrics::BetterLyrics::new()),
                Arc::new(music::youtube::YouTubeLyrics::new()),
                Arc::new(music::lrclib::LrcLib::new()),
                Arc::new(music::musixmatch::Musixmatch::new()),
            ],
        })
    }

    /// Con qué cuenta se entró, para el mensaje de inicio.
    pub fn account(&self) -> String {
        match self.session.authenticated {
            true => self.session.profile.display_name.clone(),
            false => "invitado (sin cuenta)".to_owned(),
        }
    }

    pub fn decoder(&self) -> Arc<dyn Decode> {
        self.decoder.clone()
    }

    fn api(&self) -> &Arc<dyn MusicApi> {
        &self.session.api
    }

    /// Lo que se pidió: un enlace de canción, playlist o álbum, o una búsqueda (la primera que
    /// se pueda reproducir).
    pub async fn find(&self, query: &str, requested_by: &str) -> Result<Vec<Song>> {
        let query = query.trim();
        if query.is_empty() {
            bail!("escribe qué canción quieres");
        }
        let tracks = match self.provider.link(query) {
            Some(Linked::Track(id)) => vec![self.api().track(&id).await?],
            Some(Linked::Playlist(id)) => self.api().playlist_tracks(&id).await?,
            Some(Linked::Album(id)) => self.api().album(&id).await?.tracks,
            _ => {
                let found = self.api().search(query).await?;
                found
                    .into_iter()
                    .find(|track| track.playable && track.id.is_some())
                    .into_iter()
                    .collect()
            }
        };
        let songs: Vec<Song> = tracks
            .iter()
            .filter_map(|track| Song::from_track(track, requested_by))
            .take(LIST_LIMIT)
            .collect();
        if songs.is_empty() {
            bail!("no encontré nada que se pueda reproducir para «{query}»");
        }
        Ok(songs)
    }

    /// Canciones parecidas a `song`, sin ella misma, para que la música no se acabe.
    pub async fn radio(&self, song: &Song, take: usize) -> Result<Vec<Song>> {
        let (tracks, _) = self.api().track_radio(&song.id, None).await?;
        Ok(tracks
            .iter()
            .filter_map(|track| Song::from_track(track, "Radio"))
            .filter(|found| found.id != song.id)
            .take(take)
            .collect())
    }

    /// La letra de `song` en texto, o `None` si ningún servicio la tiene.
    pub async fn lyrics(&self, song: &Song) -> Option<String> {
        let query = LyricsQuery {
            title: song.title.clone(),
            artist: song.artists.clone(),
            album: (!song.album.is_empty()).then(|| song.album.clone()),
            duration: song.duration,
            track: Some(TrackKey {
                provider: "youtube",
                id: song.id.clone(),
            }),
        };
        let asked = self.lyrics.iter().map(|source| {
            let query = query.clone();
            async move {
                match tokio::time::timeout(LYRICS_PATIENCE, source.search(&query)).await {
                    Ok(Ok(mut found)) => {
                        found.retain(|hit| music::lyrics::matched(&query, hit));
                        found
                    }
                    _ => Vec::new(),
                }
            }
        });
        let hits: Vec<LyricsHit> = join_all(asked).await.into_iter().flatten().collect();
        if music::lyrics::instrumental(&query, &hits) {
            return Some("🎻 *Instrumental*".to_owned());
        }
        let mut ranked = music::lyrics::rank(&query, hits);
        music::lyrics::reshape(&mut ranked);
        let best = ranked.into_iter().next()?;
        let text = match best.lyrics {
            Lyrics::Plain { text, .. } => text,
            Lyrics::Synced { lines } => lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        };
        let text = text.trim().to_owned();
        (!text.is_empty()).then_some(text)
    }
}

async fn guest(provider: &YouTubeProvider) -> Result<ProviderSession> {
    let sink: music::PromptSink = Arc::new(|_| {});
    let (_sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    provider
        .sign_in(SignIn::Anonymous, sink, receiver)
        .await
        .context("no se pudo abrir YouTube Music")
}

/// Las portadas de Google vienen en miniatura; se pide una de 544 px para el panel.
fn sized(cover: &str) -> String {
    match cover.rsplit_once('=') {
        Some((base, size)) if cover.contains("googleusercontent.com") && size.starts_with('w') => {
            format!("{base}=w544-h544-l90-rj")
        }
        _ => cover.to_owned(),
    }
}
