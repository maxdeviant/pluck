mod lastfm;

use std::collections::HashMap;
use std::env;
use std::time::Duration;

use chrono::{DateTime, Datelike, Utc};
use dotenv::dotenv;
use serde::Serialize;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::lastfm::LastfmFetcher;

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

    let lastfm_fetcher = LastfmFetcher::new(lastfm_user, lastfm_api_key);

    let response = lastfm_fetcher
        .fetch_tracks_page_with_cache(current_page)
        .await?;

    let total_pages = response.recent_tracks.metadata.total_pages;

    loop {
        println!("Processing page {} of {}", current_page, total_pages);

        let response = lastfm_fetcher
            .fetch_tracks_page_with_cache(current_page)
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

        if current_page % 10 == 0 {
            println!("Taking a quick break...");

            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }

    #[derive(Debug, Serialize)]
    struct YearData {
        tracks: Vec<Track>,
    }

    for (year, mut tracks) in tracks_by_year {
        tracks.sort_unstable_by(|a, b| b.listened_at.cmp(&a.listened_at));

        let mut file = File::create(format!(
            "/Users/maxdeviant/projects/data/data/last.fm/{}.toml",
            year
        ))
        .await?;
        file.write_all(toml::to_string_pretty(&YearData { tracks })?.as_bytes())
            .await?;
    }

    Ok(())
}
