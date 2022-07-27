mod types;

use std::path::Path;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub use types::*;

pub struct LastfmFetcher {
    user: String,
    api_key: String,
}

impl LastfmFetcher {
    pub fn new(user: String, api_key: String) -> Self {
        Self { user, api_key }
    }

    pub async fn fetch_tracks_page_with_cache(
        &self,
        page: i32,
    ) -> Result<GetRecentTracksResponse, Box<dyn std::error::Error>> {
        let cache_dir = Path::new(".cache/lastfm");

        if !cache_dir.exists() {
            tokio::fs::create_dir_all(cache_dir).await?;
        }

        let cached_page_path = cache_dir.join(format!("{}.json", page));

        if cached_page_path.exists() {
            println!("Fetching page {} from cache", page);

            let mut cached_page = File::open(&cached_page_path).await?;
            let mut buffer = String::new();
            cached_page.read_to_string(&mut buffer).await?;

            let response: GetRecentTracksResponse = serde_json::from_str(&buffer)?;

            Ok(response)
        } else {
            println!("Fetching page {} from last.fm", page);

            let response = self.fetch_tracks_page(page).await?;

            let mut cached_page = File::create(cached_page_path).await?;
            cached_page
                .write_all(serde_json::to_string_pretty(&response)?.as_bytes())
                .await?;

            Ok(response)
        }
    }

    pub async fn fetch_tracks_page(
        &self,
        page: i32,
    ) -> Result<GetRecentTracksResponse, Box<dyn std::error::Error>> {
        let query = {
            let limit = 200.to_string();
            let page = page.to_string();

            let query_params: querystring::QueryParams = vec![
                ("user", &self.user),
                ("api_key", &self.api_key),
                ("method", "user.getrecenttracks"),
                ("format", "json"),
                ("limit", &limit),
                ("page", &page),
            ];

            String::from(querystring::stringify(query_params).trim_end_matches("&"))
        };

        let url = format!("https://ws.audioscrobbler.com/2.0/?{}", query);
        let response = reqwest::get(url)
            .await?
            .json::<GetRecentTracksResponse>()
            .await?;

        Ok(response)
    }
}
