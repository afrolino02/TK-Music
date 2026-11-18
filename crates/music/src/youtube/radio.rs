//! A track's station, in the order YouTube ranks it and as far as YouTube goes on serving it.
//! `ytmusic::track_radio` asks for the first panel alone, and the continuation that keeps a
//! station running past it is only in the raw answer, so the request is made here.

use anyhow::Result;
use serde_json::{Value, json};
use ytmusic::nav::Nav as _;
use ytmusic::{Client, YtMusic, parse};

use crate::Track;
use crate::youtube::wire;

/// The station YouTube Music opens when a track is played on its own: the seed first, then what
/// YouTube's automix picks after it. With `from` it is the next stretch instead, which arrives
/// under a different renderer key. The seed goes with the continuation either way, since on its
/// own the token answers with most of the panel before it again.
pub(crate) async fn station(
    api: &YtMusic,
    video_id: &str,
    from: Option<&str>,
) -> Result<(Vec<Track>, Option<String>)> {
    let key = match from {
        Some(_) => "playlistPanelContinuation",
        None => "playlistPanelRenderer",
    };
    let response = api
        .execute("next", Client::Music, request(video_id, from))
        .await?;
    Ok(panel(&response, key))
}

/// The `next` request for the station seeded by `video_id`, at its start or at a continuation.
fn request(video_id: &str, continuation: Option<&str>) -> Value {
    let mut body = json!({
        "videoId": video_id,
        "playlistId": format!("RDAMVM{video_id}"),
        "enablePersistentPlaylistPanel": true,
        "tunerSettingValue": "AUTOMIX_SETTING_NORMAL",
    });
    if let Some(continuation) = continuation {
        body["continuation"] = json!(continuation);
    }
    body
}

/// The tracks of the first panel under `key`, and the continuation that follows them. A panel
/// with no radio continuation is the end of the station.
fn panel(response: &Value, key: &str) -> (Vec<Track>, Option<String>) {
    let Some(panel) = parse::find_renderers(response, key).into_iter().next() else {
        return (Vec::new(), None);
    };
    let tracks = panel
        .items(&["contents"])
        .iter()
        .map(song_side)
        .filter_map(|item| parse::panel_track(&item))
        .enumerate()
        .map(|(index, track)| wire::track(track, index as u32))
        .collect();
    let next = panel
        .items(&["continuations"])
        .iter()
        .find_map(|item| item.str_at(&["nextRadioContinuationData", "continuation"]))
        .map(str::to_string);
    (tracks, next)
}

/// The song side of a station entry. YouTube pairs a music video with its song in a wrapper, and
/// the video often comes first; its thumbnail is a frame of the clip, usually the artist, while
/// the song carries the album art. An entry with no song in it is left as it is.
fn song_side(item: &Value) -> Value {
    let Some(wrapper) = item.at(&["playlistPanelVideoWrapperRenderer"]) else {
        return item.clone();
    };
    let primary = wrapper.at(&["primaryRenderer", "playlistPanelVideoRenderer"]);
    let counterparts = wrapper
        .items(&["counterpart"])
        .iter()
        .filter_map(|side| side.at(&["counterpartRenderer", "playlistPanelVideoRenderer"]));
    primary
        .into_iter()
        .chain(counterparts)
        .find(|renderer| is_song(renderer))
        .map(|renderer| json!({ "playlistPanelVideoRenderer": renderer }))
        .unwrap_or_else(|| item.clone())
}

/// Whether a panel entry is a song (YouTube's "ATV" kind) rather than a music video.
fn is_song(renderer: &Value) -> bool {
    renderer.str_at(&[
        "navigationEndpoint",
        "watchEndpoint",
        "watchEndpointMusicSupportedConfigs",
        "watchEndpointMusicConfig",
        "musicVideoType",
    ]) == Some("MUSIC_VIDEO_TYPE_ATV")
}
