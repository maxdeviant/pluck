use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DisplayFromStr};

/// The date format used by tweets stored in a Twitter archive.
///
/// Matches the following format: `Fri Sep 28 22:03:55 +0000 2018`.
const DATE_FORMAT: &'static str = "%a %b %d %H:%M:%S %z %Y";

pub fn deserialize_date<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Utc.datetime_from_str(&s, DATE_FORMAT)
        .map_err(serde::de::Error::custom)
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct ArchivedTweet {
    #[serde_as(as = "DisplayFromStr")]
    pub id: u64,

    #[serde(deserialize_with = "deserialize_date")]
    pub created_at: DateTime<Utc>,

    pub full_text: String,
    pub entities: ArchivedTweetEntities,
}

impl ArchivedTweet {
    pub fn is_retweet(&self) -> bool {
        self.full_text.starts_with("RT @")
    }
}

#[derive(Debug, Deserialize)]
pub struct ArchivedTweetWrapper {
    pub tweet: ArchivedTweet,
}

#[derive(Debug, Deserialize)]
pub struct ArchivedTweetEntities {
    pub urls: Vec<ArchivedTweetUrlEntity>,
    pub media: Option<Vec<ArchivedTweetMediaEntity>>,
}

#[derive(Debug, Deserialize)]
pub struct WellFormedArchivedTweetUrlEntity {
    pub display_url: String,
    pub expanded_url: Option<String>,
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ArchivedTweetUrlEntity {
    WellFormed(WellFormedArchivedTweetUrlEntity),
    Malformed { url: String },
}

#[derive(Debug, Copy, Clone, Deserialize)]
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

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct ArchivedTweetMediaEntity {
    #[serde_as(as = "DisplayFromStr")]
    pub id: u64,

    #[serde(rename = "type")]
    pub media_type: MediaType,

    pub media_url_https: String,
}

pub struct TwitterArchiveImporter {
    tweet_files: Vec<PathBuf>,
    include_retweets: bool,
}

impl TwitterArchiveImporter {
    pub fn new<P: Into<PathBuf>>(tweet_files: Vec<P>) -> Self {
        Self {
            tweet_files: tweet_files.into_iter().map(Into::into).collect(),
            include_retweets: false,
        }
    }

    pub fn include_retweets(&mut self) -> &mut Self {
        self.include_retweets = true;
        self
    }

    pub fn get_tweets(&self) -> Result<Vec<ArchivedTweet>, Box<dyn std::error::Error>> {
        let mut all_tweets = Vec::new();

        for tweet_file in &self.tweet_files {
            let tweets = self.get_tweets_from_file(tweet_file)?;

            all_tweets.extend(tweets);
        }

        Ok(all_tweets)
    }

    fn get_tweets_from_file(
        &self,
        tweet_filepath: &Path,
    ) -> Result<Vec<ArchivedTweet>, Box<dyn std::error::Error>> {
        let mut tweet_file = File::open(&tweet_filepath)?;

        let mut buffer = String::new();
        tweet_file.read_to_string(&mut buffer)?;

        let buffer = buffer
            .replace("window.YTD.tweets.part0 = ", "")
            .replace("window.YTD.tweets.part1 = ", "");

        let raw_tweets: Vec<serde_json::Value> = serde_json::from_str(&buffer)?;

        let mut tweets: Vec<ArchivedTweet> = Vec::new();
        for raw_tweet in raw_tweets {
            let tweet = serde_json::from_value::<ArchivedTweetWrapper>(raw_tweet.clone());

            match tweet {
                Ok(tweet) => {
                    let tweet = tweet.tweet;

                    if self.include_retweets || !tweet.is_retweet() {
                        tweets.push(tweet);
                    }
                }
                Err(err) => eprintln!(
                    "Failed to parse tweet: {}\n\n{}",
                    err,
                    serde_json::to_string_pretty(&raw_tweet)?
                ),
            }
        }

        Ok(tweets)
    }
}
