mod lastfm;

use std::{collections::HashMap, env};

use chrono::{DateTime, Datelike, Utc};
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

    let mut tracks_by_year: HashMap<i32, Vec<Track>> = HashMap::new();

    let mut current_page = 1;

    let url = format!("https://ws.audioscrobbler.com/2.0/?method=user.getrecenttracks&user={user}&api_key={api_key}&format=json&page={page}&limit=200", user = lastfm_user, api_key = lastfm_api_key, page = current_page);
    let response = reqwest::get(url)
        .await?
        .json::<GetRecentTracksResponse>()
        .await?;

    let total_pages = response.recent_tracks.metadata.total_pages;

    loop {
        println!("Processing page {} of {}", current_page, total_pages);

        let url = format!("https://ws.audioscrobbler.com/2.0/?method=user.getrecenttracks&user={user}&api_key={api_key}&format=json&page={page}&limit=200", user = lastfm_user, api_key = lastfm_api_key, page = current_page);
        let response = reqwest::get(url)
            .await?
            .json::<GetRecentTracksResponse>()
            .await?;

        for track in response.recent_tracks.track {
            let track = Track {
                name: track.name,
                artist: track.artist.name,
                album: track.album.name,
                listened_at: track.date.timestamp,
            };

            let year = track.listened_at.year();

            tracks_by_year.entry(year).or_insert(Vec::new()).push(track);
        }

        current_page += 1;

        if current_page > total_pages {
            break;
        }
    }

    #[derive(Debug, Serialize)]
    struct YearData {
        tracks: Vec<Track>,
    }

    for (year, mut tracks) in tracks_by_year {
        tracks.sort_unstable_by(|a, b| b.listened_at.cmp(&a.listened_at));

        let mut file = File::create(format!("last.fm/{}.toml", year)).await?;
        file.write_all(toml::to_string_pretty(&YearData { tracks: tracks })?.as_bytes())
            .await?;
    }

    Ok(())
}
