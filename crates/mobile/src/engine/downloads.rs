//! Las canciones que la persona descarga a propósito para escucharlas sin internet. Se bajan
//! enteras con un reproductor aparte que nunca suena, así la música que está sonando no se corta,
//! y quedan cifradas en la bóveda como descargas: no se borran solas como la caché.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use music::{Fraction, Player};
use serde::{Deserialize, Serialize};

use super::{Engine, TrackItem, lock, signed_out};
use crate::{MobileError, failed, runtime};

/// Una canción descargada (o que se está descargando).
#[derive(Clone, Serialize, Deserialize)]
struct Saved {
    slug: String,
    track: TrackItem,
}

/// Lo descargado, guardado en el teléfono para recordarlo al volver a abrir la app.
static SAVED: LazyLock<Mutex<Vec<Saved>>> = LazyLock::new(|| {
    let saved = path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    Mutex::new(saved)
});

/// El reproductor de cada servicio que solo descarga.
static FETCHERS: LazyLock<Mutex<HashMap<String, Arc<dyn Player>>>> = LazyLock::new(Mutex::default);

fn path() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join("sonora").join("downloads.json"))
}

fn store(saved: &[Saved]) {
    let Some(path) = path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(saved) {
        let _ = std::fs::write(path, bytes);
    }
}

/// Las canciones de `slug` descargadas a propósito, la más reciente primero.
pub(super) fn downloaded(slug: &str) -> Vec<TrackItem> {
    lock(&SAVED)
        .iter()
        .rev()
        .filter(|saved| saved.slug == slug)
        .filter(|saved| super::kept_download(&saved.slug, &saved.track.id))
        .map(|saved| saved.track.clone())
        .collect()
}

impl Engine {
    /// El reproductor que descarga las canciones de `slug`, arrancándolo la primera vez.
    fn fetcher(&self, slug: &str) -> Result<Arc<dyn Player>, MobileError> {
        if let Some(player) = lock(&FETCHERS).get(slug) {
            return Ok(player.clone());
        }
        let factory = lock(&self.inner.sessions)
            .get(slug)
            .map(|session| session.playback.clone())
            .ok_or_else(|| signed_out(slug))?;
        let config = lock(&self.inner.config).clone();
        let (player, mut events) = {
            let _context = runtime().enter();
            factory.start(config)
        };
        let player: Arc<dyn Player> = Arc::from(player);
        // nadie escucha lo que dice este reproductor; se vacía para que no se acumule
        runtime().spawn(async move { while events.next().await.is_some() {} });
        lock(&FETCHERS).insert(slug.to_owned(), player.clone());
        Ok(player)
    }
}

#[uniffi::export]
impl Engine {
    /// Descarga la canción entera para escucharla sin internet. Tarda lo que tarde en bajar, así
    /// que se llama fuera del hilo principal. Una canción ya descargada responde al instante.
    pub fn download_track(&self, slug: String, track: TrackItem) -> Result<(), MobileError> {
        let player = self.fetcher(&slug)?;
        runtime()
            .block_on(player.download(&track.id, Fraction::default()))
            .map_err(|error| match error.to_string().contains("cannot download") {
                true => MobileError::Failed {
                    message: "Las canciones de este servicio no se pueden descargar.".to_owned(),
                },
                false => failed(error),
            })?;
        let mut saved = lock(&SAVED);
        saved.retain(|kept| !(kept.slug == slug && kept.track.id == track.id));
        saved.push(Saved { slug, track });
        store(&saved);
        Ok(())
    }

    /// Borra del teléfono una canción descargada.
    pub fn remove_download(&self, slug: String, id: String) {
        if let Ok(player) = self.fetcher(&slug) {
            player.discard(&id);
        }
        let mut saved = lock(&SAVED);
        saved.retain(|kept| !(kept.slug == slug && kept.track.id == id));
        store(&saved);
    }

    /// Si la canción está descargada a propósito en el teléfono.
    pub fn is_downloaded(&self, slug: String, id: String) -> bool {
        lock(&SAVED)
            .iter()
            .any(|kept| kept.slug == slug && kept.track.id == id)
            && super::kept_download(&slug, &id)
    }

    /// Las canciones de `slug` descargadas a propósito, la más reciente primero.
    pub fn downloaded_tracks(&self, slug: String) -> Vec<TrackItem> {
        downloaded(&slug)
    }
}
