//! La configuración del bot: el token de Discord, guardado en el PC la primera vez que se abre.

use std::io::{self, BufRead as _, Write as _};
use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

/// El nombre del canal de texto donde cualquier mensaje (o un webhook, como el de la app
/// TK-Music) se toma como una canción para la cola.
pub const QUEUE_CHANNEL: &str = "tk-music";

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    /// El token del bot, del portal de desarrolladores de Discord.
    #[serde(default)]
    pub token: String,
    /// El volumen con el que arranca cada servidor, en porcentaje.
    #[serde(default = "default_volume")]
    pub volume: u8,
}

fn default_volume() -> u8 {
    60
}

impl Config {
    /// Dónde se guarda: `%APPDATA%\TK-Music-Bot\config.json` en Windows.
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("TK-Music-Bot")
            .join("config.json")
    }

    /// Lee la configuración; si todavía no hay token, lo pide en la consola y lo guarda.
    pub fn load() -> Result<Self> {
        let path = Self::path();
        let mut config: Config = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("{} está dañado; bórralo y vuelve a abrir el bot", path.display()))?,
            Err(_) => Config {
                volume: default_volume(),
                ..Config::default()
            },
        };
        if let Ok(token) = std::env::var("DISCORD_TOKEN") {
            config.token = token;
        }
        if config.token.trim().is_empty() {
            config.token = ask_token()?;
            config.save()?;
            println!("Guardado en {}", path.display());
        }
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("no se pudo guardar {}", path.display()))
    }

    /// Borra el token guardado, para pedirlo otra vez al abrir.
    pub fn forget_token() -> Result<()> {
        let mut config = Self::load_quietly();
        config.token.clear();
        config.save()
    }

    fn load_quietly() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
}

fn ask_token() -> Result<String> {
    println!();
    println!("Primera vez: falta el token del bot de Discord.");
    println!("Sácalo en https://discord.com/developers/applications → tu app → Bot → Reset Token.");
    print!("Pega el token aquí y presiona Enter: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let token = line.trim().trim_matches('"').to_owned();
    if token.is_empty() {
        bail!("no se pegó ningún token");
    }
    Ok(token)
}
