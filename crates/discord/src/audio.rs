//! El puente entre el decodificador de TK-Music y songbird: la canción se descarga y se
//! decodifica cuando le toca sonar (o un poco antes), y sus muestras pasan a Discord como PCM.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use music::engine::Decode;
use songbird::input::{AudioStream, AudioStreamError, AuxMetadata, Compose, Input, RawAdapter};
use symphonia::core::io::MediaSource;

use crate::music::Song;

/// Cuántas muestras viajan juntas del hilo que decodifica al mezclador.
const CHUNK: usize = 4096;
/// Cuántos trozos se adelantan como mucho (unos 3 segundos en estéreo a 44,1 kHz).
const AHEAD: usize = 64;
/// Cuánto espera el mezclador por música antes de mandar un silencio corto, para no trabar la
/// llamada si la red va lenta.
const WAIT: Duration = Duration::from_millis(15);
/// El silencio que se manda cuando la música no llegó a tiempo, en bytes (unos 5 ms).
const GAP: usize = 4 * 2 * 256;

/// Una canción por abrir: songbird la crea justo antes de que suene.
pub struct Lazy {
    song: Song,
    decoder: Arc<dyn Decode>,
}

impl Lazy {
    pub fn input(song: Song, decoder: Arc<dyn Decode>) -> Input {
        Input::Lazy(Box::new(Self { song, decoder }))
    }
}

#[async_trait]
impl Compose for Lazy {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let decoded = self
            .decoder
            .decode(&self.song.id)
            .await
            .map_err(|error| AudioStreamError::Fail(error.into()))?;
        let rate = decoded.source.sample_rate().get();
        let channels = u32::from(decoded.source.channels().get());
        let (sender, receiver) = sync_channel(AHEAD);
        let title = self.song.title.clone();
        std::thread::Builder::new()
            .name("tk-decode".into())
            .spawn(move || pump(decoded.source, sender, &title))
            .map_err(|error| AudioStreamError::Fail(error.into()))?;
        let pcm = Pcm {
            chunks: Mutex::new(receiver),
            current: Vec::new(),
            at: 0,
            done: false,
        };
        Ok(AudioStream {
            input: Box::new(RawAdapter::new(pcm, rate, channels)),
        })
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        Ok(AuxMetadata {
            title: Some(self.song.title.clone()),
            artist: Some(self.song.artists.clone()),
            album: Some(self.song.album.clone()),
            duration: Some(self.song.duration),
            source_url: Some(self.song.url()),
            thumbnail: self.song.cover.clone(),
            ..AuxMetadata::default()
        })
    }
}

/// Decodifica la canción entera, trozo a trozo, hasta que se acaba o nadie la escucha.
fn pump(source: Box<dyn rodio::Source + Send>, sender: SyncSender<Vec<f32>>, title: &str) {
    let mut chunk = Vec::with_capacity(CHUNK);
    for sample in source {
        chunk.push(sample);
        if chunk.len() == CHUNK {
            let full = std::mem::replace(&mut chunk, Vec::with_capacity(CHUNK));
            if sender.send(full).is_err() {
                log::debug!("audio: «{title}» se dejó de escuchar");
                return;
            }
        }
    }
    if !chunk.is_empty() {
        let _ = sender.send(chunk);
    }
}

/// Las muestras como bytes f32 intercalados, que es lo que lee `RawAdapter`.
struct Pcm {
    chunks: Mutex<Receiver<Vec<f32>>>,
    current: Vec<u8>,
    at: usize,
    done: bool,
}

impl Read for Pcm {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.at >= self.current.len() {
            if self.done {
                return Ok(0);
            }
            let next = self
                .chunks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .recv_timeout(WAIT);
            match next {
                Ok(samples) => {
                    self.current = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
                    self.at = 0;
                }
                Err(RecvTimeoutError::Timeout) => {
                    // la red va lenta: un silencio cortito en vez de trabar la llamada
                    let gap = buf.len().min(GAP) / 8 * 8;
                    if gap > 0 {
                        buf[..gap].fill(0);
                        return Ok(gap);
                    }
                    // tan poco espacio que no cabe ni un silencio entero: se espera la música
                    let waited = self
                        .chunks
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .recv();
                    match waited {
                        Ok(samples) => {
                            self.current =
                                samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
                            self.at = 0;
                        }
                        Err(_) => {
                            self.done = true;
                            return Ok(0);
                        }
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.done = true;
                    return Ok(0);
                }
            }
        }
        let available = &self.current[self.at..];
        let count = available.len().min(buf.len());
        buf[..count].copy_from_slice(&available[..count]);
        self.at += count;
        Ok(count)
    }
}

impl Seek for Pcm {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

impl MediaSource for Pcm {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}
