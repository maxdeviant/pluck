use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr, TimestampSeconds};

/// The response from the [`user.getRecentTracks`](https://www.last.fm/api/show/user.getRecentTracks) method.
#[derive(Debug, Deserialize)]
pub struct GetRecentTracksResponse {
    #[serde(rename = "recenttracks")]
    pub recent_tracks: RecentTracks,
}

#[derive(Debug, Deserialize)]
pub struct RecentTracks {
    pub track: Vec<Track>,

    #[serde(rename = "@attr")]
    pub metadata: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct Track {
    pub name: String,
    pub artist: Artist,
    pub album: Album,
    pub date: TrackDate,
}

#[derive(Debug, Deserialize)]
pub struct Artist {
    #[serde(rename = "#text")]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    #[serde(rename = "#text")]
    pub name: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct TrackDate {
    #[serde_as(as = "TimestampSeconds<String>")]
    #[serde(rename = "uts")]
    pub timestamp: DateTime<Utc>,

    #[serde(rename = "#text")]
    pub text: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde_as(as = "DisplayFromStr")]
    pub page: i32,

    #[serde_as(as = "DisplayFromStr")]
    pub total_pages: i32,

    #[serde_as(as = "DisplayFromStr")]
    pub per_page: i32,

    #[serde_as(as = "DisplayFromStr")]
    pub total: i32,
}
