//! Lo que se ve en Discord: el panel de «Sonando ahora» con su barra y sus botones, la cola,
//! la letra y los avisos.

use std::time::Duration;

use poise::serenity_prelude as serenity;
use serenity::{
    ButtonStyle, ChannelId, Colour, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter,
    Http, MessageId,
};

use crate::music::Song;

/// El morado de TK-Music.
pub const COLOR: Colour = Colour::new(0x8B5CF6);
/// Cuántos tramos tiene la barra de progreso.
const BAR: usize = 16;
/// Lo más largo que Discord deja en la descripción de un embed.
const DESCRIPTION_LIMIT: usize = 4000;

/// Los botones del panel.
pub mod button {
    pub const TOGGLE: &str = "tk:toggle";
    pub const SKIP: &str = "tk:skip";
    pub const STOP: &str = "tk:stop";
    pub const RADIO: &str = "tk:radio";
    pub const LYRICS: &str = "tk:lyrics";
    pub const QUEUE: &str = "tk:queue";
}

/// El panel de la canción que suena.
pub fn now_playing(
    song: &Song,
    position: Duration,
    paused: bool,
    radio: bool,
    waiting: usize,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    let mut description = format!("**{}**", song.artists);
    if !song.album.is_empty() && song.album != song.title {
        description.push_str(&format!(" · {}", song.album));
    }
    description.push_str(&format!(
        "\n\n{} `{}` {} `{}`",
        if paused { "⏸️" } else { "▶️" },
        clock(position),
        bar(position, song.duration),
        clock(song.duration),
    ));
    let mut footer = format!("Pedida por {}", song.requested_by);
    match waiting {
        0 => footer.push_str(" · no hay más en la cola"),
        1 => footer.push_str(" · 1 en la cola"),
        n => footer.push_str(&format!(" · {n} en la cola")),
    }
    if radio {
        footer.push_str(" · 📻 radio");
    }
    let mut embed = CreateEmbed::new()
        .colour(COLOR)
        .author(serenity::CreateEmbedAuthor::new("Sonando ahora · TK-Music"))
        .title(&song.title)
        .url(song.url())
        .description(description)
        .footer(CreateEmbedFooter::new(footer));
    if let Some(cover) = &song.cover {
        embed = embed.thumbnail(cover);
    }
    let controls = CreateActionRow::Buttons(vec![
        CreateButton::new(button::TOGGLE)
            .emoji(if paused { '▶' } else { '⏸' })
            .style(ButtonStyle::Primary),
        CreateButton::new(button::SKIP).emoji('⏭').style(ButtonStyle::Secondary),
        CreateButton::new(button::STOP).emoji('⏹').style(ButtonStyle::Danger),
        CreateButton::new(button::QUEUE).label("Cola").style(ButtonStyle::Secondary),
    ]);
    let extras = CreateActionRow::Buttons(vec![
        CreateButton::new(button::RADIO)
            .label(if radio { "Radio: sí" } else { "Radio: no" })
            .style(if radio { ButtonStyle::Success } else { ButtonStyle::Secondary }),
        CreateButton::new(button::LYRICS).label("Letra").style(ButtonStyle::Secondary),
    ]);
    (embed, vec![controls, extras])
}

/// Deja el panel viejo como un aviso, sin botones.
pub async fn retire(http: &Http, channel: ChannelId, message: MessageId, text: &str) {
    let embed = CreateEmbed::new().colour(COLOR).description(text);
    let edit = serenity::EditMessage::new().embed(embed).components(Vec::new());
    let _ = channel.edit_message(http, message, edit).await;
}

/// El aviso de lo que se agregó a la cola.
pub fn added(songs: &[Song], playing_now: bool) -> CreateEmbed {
    let first = &songs[0];
    let mut embed = CreateEmbed::new().colour(COLOR);
    if songs.len() == 1 {
        let what = if playing_now { "▶️ Va a sonar" } else { "➕ Agregada a la cola" };
        embed = embed
            .description(format!("{what}: **[{}]({})**\n{}", first.title, first.url(), first.artists));
        if let Some(cover) = &first.cover {
            embed = embed.thumbnail(cover);
        }
    } else {
        let total: Duration = songs.iter().map(|song| song.duration).sum();
        embed = embed.description(format!(
            "➕ Agregué **{} canciones** ({}), empezando por **{}**.",
            songs.len(),
            clock(total),
            first.title
        ));
    }
    embed
}

/// La cola: lo que suena y lo que viene.
pub fn queue(current: Option<&Song>, upcoming: &[Song]) -> CreateEmbed {
    let mut text = String::new();
    match current {
        Some(song) => text.push_str(&format!("**Sonando:** [{}]({}) — {}\n\n", song.title, song.url(), song.artists)),
        None => text.push_str("No suena nada.\n\n"),
    }
    if upcoming.is_empty() {
        text.push_str("No hay más canciones en la cola.");
    } else {
        for (index, song) in upcoming.iter().enumerate() {
            let line = format!("`{}.` {} — {} `{}`\n", index + 1, song.title, song.artists, clock(song.duration));
            if text.len() + line.len() > DESCRIPTION_LIMIT - 40 {
                text.push_str(&format!("…y {} más.", upcoming.len() - index));
                break;
            }
            text.push_str(&line);
        }
    }
    CreateEmbed::new().colour(COLOR).title("Cola de TK-Music").description(text)
}

/// La letra, partida en embeds si es muy larga.
pub fn lyrics(song: &Song, text: &str) -> Vec<CreateEmbed> {
    let mut pages = Vec::new();
    let mut page = String::new();
    for line in text.lines() {
        if page.len() + line.len() + 1 > DESCRIPTION_LIMIT {
            pages.push(std::mem::take(&mut page));
        }
        page.push_str(line);
        page.push('\n');
    }
    if !page.trim().is_empty() {
        pages.push(page);
    }
    pages
        .into_iter()
        .take(3)
        .enumerate()
        .map(|(index, page)| {
            let embed = CreateEmbed::new().colour(COLOR).description(page);
            match index {
                0 => embed.title(format!("📝 {} — {}", song.title, song.artists)),
                _ => embed,
            }
        })
        .collect()
}

/// Un aviso corto con el color de TK-Music.
pub fn note(text: impl Into<String>) -> CreateEmbed {
    CreateEmbed::new().colour(COLOR).description(text.into())
}

/// `1:23`, o `1:02:03` si pasa de una hora.
pub fn clock(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    match hours {
        0 => format!("{minutes}:{seconds:02}"),
        _ => format!("{hours}:{minutes:02}:{seconds:02}"),
    }
}

fn bar(position: Duration, length: Duration) -> String {
    let fraction = match length.is_zero() {
        true => 0.0,
        false => (position.as_secs_f64() / length.as_secs_f64()).clamp(0.0, 1.0),
    };
    let knob = ((fraction * (BAR - 1) as f64).round() as usize).min(BAR - 1);
    (0..BAR).map(|index| if index == knob { '🔘' } else { '▬' }).collect()
}
