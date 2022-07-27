mod lastfm;

use std::env;

use chrono::{DateTime, Utc};
use dotenv::dotenv;
use serde::Serialize;
use tokio::{fs::File, io::AsyncWriteExt};

use lastfm::GetRecentTracksResponse;

#[derive(Debug, Serialize)]
struct Track {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub listened_at: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let lastfm_user = env::var("LASTFM_USER")?;
    let lastfm_api_key = env::var("LASTFM_API_KEY")?;

    let url = format!("https://ws.audioscrobbler.com/2.0/?method=user.getrecenttracks&user={user}&api_key={api_key}&format=json", user = lastfm_user, api_key = lastfm_api_key);
    let response = reqwest::get(url)
        .await?
        .json::<GetRecentTracksResponse>()
        .await?;

    let mut formatted_tracks = Vec::new();

    for track in response.recent_tracks.track {
        let track = Track {
            name: track.name,
            artist: track.artist.name,
            album: track.album.name,
            listened_at: track.date.timestamp,
        };

        formatted_tracks.push(track);
    }

    #[derive(Debug, Serialize)]
    struct Output {
        tracks: Vec<Track>,
    }

    let mut file = File::create("2022.toml").await?;
    file.write_all(
        toml::to_string_pretty(&Output {
            tracks: formatted_tracks,
        })?
        .as_bytes(),
    )
    .await?;

    Ok(())
}
