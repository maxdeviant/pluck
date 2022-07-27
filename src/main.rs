mod lastfm;

use lastfm::GetRecentTracksResponse;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const LASTFM_USER: &'static str = "";
    const LASTFM_API_KEY: &'static str = "";

    let url = format!("https://ws.audioscrobbler.com/2.0/?method=user.getrecenttracks&user={user}&api_key={api_key}&format=json", user = LASTFM_USER, api_key = LASTFM_API_KEY);
    let response = reqwest::get(url)
        .await?
        .json::<GetRecentTracksResponse>()
        .await?;

    for track in response.recent_tracks.track {
        let x = dbg!(track);
    }

    Ok(())
}
