mod lastfm;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Datelike, Utc};
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use indexmap::set::IndexSet;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::lastfm::{LastfmFetcher, PlayedOrNowPlayingTrack};

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Track {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub listened_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct YearData {
    tracks: IndexSet<Track>,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Tweet {
    pub id: u64,
    pub created_at: DateTime<Utc>,
    pub text: String,
    pub entities: Option<TweetEntities>,
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct TweetEntities {
    pub urls: Option<Vec<TweetUrlEntity>>,
    pub media: Option<Vec<TweetMediaEntity>>,
}

impl TweetEntities {
    pub fn is_empty(&self) -> bool {
        self.urls.is_none() && self.media.is_none()
    }

    pub fn into_option(self) -> Option<TweetEntities> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct TweetUrlEntity {
    pub display_url: String,
    pub expanded_url: Option<String>,
    pub url: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaType {
    #[serde(rename = "photo")]
    Photo,

    #[serde(rename = "video")]
    Video,

    #[serde(rename = "animated_gif")]
    Gif,
}

impl From<egg_mode::entities::MediaType> for MediaType {
    fn from(value: egg_mode::entities::MediaType) -> Self {
        match value {
            egg_mode::entities::MediaType::Photo => MediaType::Photo,
            egg_mode::entities::MediaType::Video => MediaType::Video,
            egg_mode::entities::MediaType::Gif => MediaType::Gif,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct TweetMediaEntity {
    pub id: u64,
    pub r#type: MediaType,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TwitterYearData {
    tweets: IndexSet<Tweet>,
}

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Lastfm {
        output_dir: PathBuf,

        #[clap(short, long, action)]
        full_sync: bool,
    },
    Twitter {
        output_dir: PathBuf,

        #[clap(short, long, action)]
        full_sync: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let args = Args::parse();

    match args.command {
        Command::Lastfm {
            output_dir,
            full_sync,
        } => {
            let lastfm_user = env::var("LASTFM_USER")?;
            let lastfm_api_key = env::var("LASTFM_API_KEY")?;

            let mut tracks_by_year: HashMap<i32, IndexSet<Track>> = HashMap::new();

            let mut current_page = 1;

            let latest_year_data = if !full_sync {
                get_latest_year_data(&output_dir).await?
            } else {
                None
            };

            if let Some((latest_year, latest_year_data)) = latest_year_data {
                tracks_by_year.insert(latest_year, latest_year_data.tracks);
            }

            let lastfm_fetcher = LastfmFetcher::new(lastfm_user, lastfm_api_key);

            let response = if full_sync {
                lastfm_fetcher
                    .fetch_tracks_page_with_cache(current_page)
                    .await?
            } else {
                lastfm_fetcher.fetch_tracks_page(current_page).await?
            };

            let total_pages = response.recent_tracks.metadata.total_pages;

            'fetch_tracks: loop {
                println!("Processing page {} of {}", current_page, total_pages);

                let response = if full_sync {
                    lastfm_fetcher
                        .fetch_tracks_page_with_cache(current_page)
                        .await?
                } else {
                    lastfm_fetcher.fetch_tracks_page(current_page).await?
                };

                for track in
                    response
                        .recent_tracks
                        .track
                        .into_iter()
                        .filter_map(|track| match track {
                            PlayedOrNowPlayingTrack::Played(track) => Some(track),
                            PlayedOrNowPlayingTrack::NowPlaying(_) => None,
                        })
                {
                    let track = Track {
                        name: track.name,
                        artist: track.artist.name,
                        album: track.album.name,
                        listened_at: track.date.timestamp,
                    };

                    let year = track.listened_at.year();
                    let is_new_track = tracks_by_year
                        .entry(year)
                        .or_insert(IndexSet::new())
                        .insert(track);

                    if !is_new_track {
                        break 'fetch_tracks;
                    }
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

            for (year, mut tracks) in tracks_by_year {
                tracks.sort_unstable_by(|a, b| match b.listened_at.cmp(&a.listened_at) {
                    Ordering::Equal => b.name.cmp(&a.name),
                    ord => ord,
                });

                let mut file = File::create(output_dir.join(format!("{}.toml", year))).await?;
                file.write_all(toml::to_string_pretty(&YearData { tracks })?.as_bytes())
                    .await?;
            }
        }
        Command::Twitter {
            output_dir,
            full_sync,
        } => {
            let twitter_consumer_key = env::var("TWITTER_CONSUMER_KEY")?;
            let twitter_consumer_secret = env::var("TWITTER_CONSUMER_SECRET")?;

            let mut tweets_by_year: HashMap<i32, IndexSet<Tweet>> = HashMap::new();

            let consumer_token =
                egg_mode::KeyPair::new(twitter_consumer_key, twitter_consumer_secret);

            let token = egg_mode::auth::bearer_token(&consumer_token).await?;

            let timeline = egg_mode::tweet::user_timeline("maxdeviant", false, false, &token)
                .with_page_size(200);

            let (mut timeline, mut feed) = timeline.start().await?;

            'fetch_tweets: loop {
                for tweet in feed.response.clone() {
                    let tweet = Tweet {
                        id: tweet.id,
                        text: tweet.text,
                        entities: TweetEntities {
                            urls: {
                                let urls = tweet
                                    .entities
                                    .urls
                                    .into_iter()
                                    .map(|entity| TweetUrlEntity {
                                        display_url: entity.display_url,
                                        expanded_url: entity.expanded_url,
                                        url: entity.url,
                                    })
                                    .collect::<Vec<_>>();

                                if urls.is_empty() {
                                    None
                                } else {
                                    Some(urls)
                                }
                            },
                            media: tweet.entities.media.map(|media| {
                                media
                                    .into_iter()
                                    .map(|entity| TweetMediaEntity {
                                        id: entity.id,
                                        r#type: entity.media_type.into(),
                                        url: entity.media_url_https,
                                    })
                                    .collect()
                            }),
                        }
                        .into_option(),
                        created_at: tweet.created_at,
                    };

                    let year = tweet.created_at.year();
                    let is_new_tweet = tweets_by_year
                        .entry(year)
                        .or_insert(IndexSet::new())
                        .insert(tweet);

                    if !is_new_tweet {
                        break 'fetch_tweets;
                    }
                }

                (timeline, feed) = timeline.older(None).await?;

                tokio::time::sleep(Duration::from_millis(1000)).await;
            }

            for (year, mut tweets) in tweets_by_year {
                tweets.sort_unstable_by(|a, b| b.id.cmp(&a.id));

                let mut file = File::create(output_dir.join(format!("{}.toml", year))).await?;
                file.write_all(toml::to_string_pretty(&TwitterYearData { tweets })?.as_bytes())
                    .await?;
            }
        }
    }

    Ok(())
}

async fn get_latest_year_data(
    target_dir: &Path,
) -> Result<Option<(i32, YearData)>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(target_dir)? {
        let entry = entry?;

        let path = entry.path();
        if path.is_file() && path.extension() == Some(OsStr::new("toml")) {
            files.push(path);
        }
    }

    files.sort_unstable();

    if let Some(filepath) = files.pop() {
        let mut file = File::open(&filepath).await?;
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).await?;

        let year: i32 = filepath
            .file_stem()
            .expect("no filename")
            .to_str()
            .expect("invalid filename")
            .parse()?;
        let year_data: YearData = toml::from_str(&buffer)?;

        Ok(Some((year, year_data)))
    } else {
        Ok(None)
    }
}
