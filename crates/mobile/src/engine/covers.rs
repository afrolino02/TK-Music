//! La portada del álbum para lo que suena cuando YouTube Music solo dio la miniatura de un video
//! musical, que es un cuadro del clip (casi siempre el artista) y no la carátula. Se busca la
//! versión «canción» del mismo tema y se toma su portada.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use super::{Inner, google_sized, lock};
use crate::runtime;

/// Cuántas respuestas se recuerdan antes de empezar de cero.
const REMEMBERED: usize = 500;

/// La portada hallada para cada canción (`None`: se buscó y no hubo una mejor).
static FOUND: LazyLock<Mutex<HashMap<String, Option<String>>>> = LazyLock::new(Mutex::default);

/// Si lo que suena trae la miniatura de un video, busca en segundo plano la portada del álbum y
/// la pone en la cola, para que la muestren el reproductor, la barra y la lista.
pub(super) fn fix(inner: &Arc<Inner>) {
    let (slug, track) = {
        let queue = lock(&inner.queue);
        let (Some(slug), Some(track)) =
            (queue.slug.clone(), queue.tracks.get(queue.index).cloned())
        else {
            return;
        };
        (slug, track)
    };
    if !track.cover.as_deref().is_some_and(is_video_frame) {
        return;
    }
    if let Some(known) = lock(&FOUND).get(&track.id).cloned() {
        if let Some(cover) = known {
            apply(inner, &track.id, cover);
        }
        return;
    }
    let Ok(api) = inner.api(&slug) else {
        return;
    };
    let this = Arc::downgrade(inner);
    runtime().spawn(async move {
        let query = format!("{} {}", track.title, track.artists);
        let found = api
            .search(&query)
            .await
            .ok()
            .and_then(|songs| album_art(&track.title, &songs));
        {
            let mut memory = lock(&FOUND);
            if memory.len() > REMEMBERED {
                memory.clear();
            }
            memory.insert(track.id.clone(), found.clone());
        }
        if let (Some(cover), Some(inner)) = (found, this.upgrade()) {
            apply(&inner, &track.id, cover);
        }
    });
}

/// Si la imagen es la miniatura de un video de YouTube y no una carátula.
fn is_video_frame(cover: &str) -> bool {
    cover.contains("ytimg.com/")
}

/// La carátula de la primera canción de la búsqueda que es el mismo tema que el video: el título
/// del video («Artista - Tema (Video Oficial)») contiene el de la canción, o al revés.
fn album_art(title: &str, songs: &[music::Track]) -> Option<String> {
    // Con espacios a los lados, para comparar palabras enteras («yo» no está en «mayores»).
    let title = format!(" {} ", plain(title));
    songs
        .iter()
        .take(5)
        .filter(|song| {
            song.cover
                .as_deref()
                .is_some_and(|cover| !is_video_frame(cover))
        })
        .find(|song| {
            let name = format!(" {} ", plain(&song.name));
            name.trim() != "" && (title.contains(&name) || name.contains(&title))
        })
        .and_then(|song| song.cover.clone())
}

/// Un título para comparar: en minúsculas, sin lo que va entre paréntesis o corchetes y solo
/// con letras, números y espacios sencillos.
fn plain(text: &str) -> String {
    let mut depth = 0u32;
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
        }
    }
    out.trim().to_owned()
}

/// Pone la portada en cada copia de la canción en la cola y avisa a la app.
fn apply(inner: &Inner, id: &str, cover: String) {
    let cover = google_sized(&cover, 400).unwrap_or(cover);
    let changed = {
        let mut queue = lock(&inner.queue);
        let mut changed = false;
        for track in queue.tracks.iter_mut().filter(|track| track.id == id) {
            if track.cover.as_deref() != Some(cover.as_str()) {
                track.cover = Some(cover.clone());
                changed = true;
            }
        }
        changed
    };
    if changed {
        inner.notify();
    }
}
