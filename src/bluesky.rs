use atrium_api::agent::store::MemorySessionStore;
use atrium_api::agent::AtpAgent;
use atrium_api::app::bsky::feed;
use atrium_api::app::bsky::feed::defs::{FeedViewPostReasonRefs, ReplyRefParentRefs};
use atrium_api::types::string::{AtIdentifier, Handle};
use atrium_api::types::{LimitedNonZeroU8, TryFromUnknown, Union};
use atrium_xrpc_client::reqwest::{ReqwestClient, ReqwestClientBuilder};

use crate::{BlueskyPost, BlueskyPostReply};

pub(crate) struct FetchPostsOutput {
    pub posts: Vec<BlueskyPost>,
    pub cursor: Option<String>,
}

pub(crate) struct BlueskyFetcher {
    client: AtpAgent<MemorySessionStore, ReqwestClient>,
    handle: String,
    app_password: String,
}

impl BlueskyFetcher {
    pub fn new(handle: String, app_password: String) -> Self {
        Self {
            client: AtpAgent::new(
                ReqwestClientBuilder::new("https://bsky.social")
                    .client(reqwest::Client::default())
                    .build(),
                MemorySessionStore::default(),
            ),
            handle,
            app_password,
        }
    }

    pub async fn fetch_posts(
        &mut self,
        cursor: Option<String>,
    ) -> Result<FetchPostsOutput, Box<dyn std::error::Error>> {
        if self.client.get_session().await.is_none() {
            self.client.login(&self.handle, &self.app_password).await?;
        }

        use atrium_api::app::bsky::feed::get_author_feed::{self};

        let response = self
            .client
            .api
            .app
            .bsky
            .feed
            .get_author_feed(
                get_author_feed::ParametersData {
                    actor: AtIdentifier::Handle(Handle::new(self.handle.clone()).unwrap()),
                    cursor,
                    limit: Some(LimitedNonZeroU8::<100>::MAX),
                    filter: None,
                }
                .into(),
            )
            .await?;

        let cursor = response.cursor.clone();

        let mut posts = Vec::new();

        for feed_view_post in &response.feed {
            let is_repost = if let Some(reason) = &feed_view_post.reason {
                match reason {
                    Union::Refs(FeedViewPostReasonRefs::ReasonRepost(_)) => true,
                    Union::Unknown(_) => false,
                }
            } else {
                false
            };

            if is_repost {
                continue;
            }

            let in_reply_to = feed_view_post.reply.as_ref().and_then(|reply| {
                let parent = match &reply.parent {
                    Union::Refs(parent) => parent,
                    Union::Unknown(_) => return None,
                };

                match parent {
                    ReplyRefParentRefs::PostView(parent) => Some(BlueskyPostReply {
                        uri: parent.uri.clone(),
                        author_did: parent.author.did.to_string(),
                        author_handle: parent.author.handle.to_string(),
                    }),
                    ReplyRefParentRefs::NotFoundPost(_) | ReplyRefParentRefs::BlockedPost(_) => {
                        None
                    }
                }
            });

            let post = &feed_view_post.post;
            let record = feed::post::RecordData::try_from_unknown(post.record.clone())?;

            posts.push(BlueskyPost {
                uri: post.uri.clone(),
                text: record.text,
                created_at: record.created_at.as_ref().to_utc(),
                in_reply_to,
            });
        }

        Ok(FetchPostsOutput { posts, cursor })
    }
}
