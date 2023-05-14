use async_trait::async_trait;
use atrium_api::xrpc;
use http::{Request, Response};

use crate::BlueskyPost;

pub(crate) struct FetchPostsOutput {
    pub posts: Vec<BlueskyPost>,
    pub cursor: Option<String>,
}

pub(crate) struct BlueskyFetcher {
    client: BlueskyClient,
    handle: String,
    app_password: String,
}

impl BlueskyFetcher {
    pub fn new(handle: String, app_password: String) -> Self {
        Self {
            client: BlueskyClient::default(),
            handle,
            app_password,
        }
    }

    pub async fn fetch_posts(
        &mut self,
        cursor: Option<String>,
    ) -> Result<FetchPostsOutput, Box<dyn std::error::Error>> {
        use atrium_api::com::atproto::server::create_session::{CreateSession, Input};

        let session = self
            .client
            .create_session(Input {
                identifier: self.handle.clone(),
                password: self.app_password.clone(),
            })
            .await?;

        self.client.set_auth(session.access_jwt);

        use atrium_api::app::bsky::feed::get_author_feed::{self, GetAuthorFeed};

        let response = self
            .client
            .get_author_feed(get_author_feed::Parameters {
                actor: session.did,
                cursor,
                limit: Some(100),
            })
            .await?;

        let cursor = response.cursor.clone();

        let response = dbg!(response);

        use atrium_api::records::Record;

        let mut posts = Vec::new();

        for post in response.feed {
            let post = post.post;

            match post.record {
                Record::AppBskyFeedPost(record) => {
                    posts.push(BlueskyPost {
                        uri: post.uri,
                        text: record.text,
                        created_at: record.created_at.parse()?,
                    });
                }
                _ => {}
            }
        }

        Ok(FetchPostsOutput { posts, cursor })
    }
}

#[derive(Default)]
struct BlueskyClient {
    http_client: reqwest::Client,
    auth: Option<String>,
}

impl BlueskyClient {
    pub fn set_auth(&mut self, auth: String) {
        self.auth = Some(auth)
    }
}

#[async_trait]
impl xrpc::HttpClient for BlueskyClient {
    async fn send(
        &self,
        req: Request<Vec<u8>>,
    ) -> std::result::Result<Response<Vec<u8>>, Box<dyn std::error::Error>> {
        let res = self.http_client.execute(req.try_into()?).await?;
        let mut builder = http::Response::builder().status(res.status());
        for (k, v) in res.headers() {
            builder = builder.header(k, v);
        }
        builder
            .body(res.bytes().await?.to_vec())
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl xrpc::XrpcClient for BlueskyClient {
    fn host(&self) -> &str {
        "https://bsky.social"
    }

    fn auth(&self) -> Option<&str> {
        self.auth.as_deref()
    }
}

atrium_api::impl_traits!(BlueskyClient);
