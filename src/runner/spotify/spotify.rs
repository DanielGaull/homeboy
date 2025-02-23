use std::{env, error::Error};

use rspotify::{model::{Country, Id, Market, SearchResult, SearchType}, prelude::{BaseClient, OAuthClient}, scopes, AuthCodeSpotify, Credentials, OAuth};

pub struct Spotify {
    client: AuthCodeSpotify,
}

impl Spotify {
    pub async fn init() -> Result<Self, Box<dyn Error>> {
        let redirect_url = env::var(String::from("sp_redirect_uri"))?;
        let client_id = env::var(String::from("sp_client_id"))?;
        let client_secret = env::var(String::from("sp_client_secret"))?;
        let creds = Credentials::new(&client_id, &client_secret);
        let mut oauth = OAuth::default();
        oauth.redirect_uri = redirect_url;
        oauth.scopes = scopes!("user-read-playback-state", "user-modify-playback-state", "user-library-read");
        // let code = env::var(String::from("sp_code"))?;
        let spotify = AuthCodeSpotify::new(creds, oauth);
        let url = spotify.get_authorize_url(false).unwrap();
        spotify.prompt_for_token(&url).await?;

        Ok(Spotify {
            client: spotify
        })
    }
    
    pub async fn get_song(&mut self, query: String) -> Result<Option<String>, Box<dyn Error>> {
        let result = self.client.search(
            &query, 
            SearchType::Track, 
            Some(Market::Country(Country::UnitedStates)), 
            None, 
            Some(1), 
            None,
        ).await?;
        if let SearchResult::Tracks(page) = result {
            if let Some(track) = page.items.get(0) {
                if let Some(id) = &track.id {
                    return Ok(Some(String::from(id.id())));
                }
            }
        }
        Ok(None)
    }
}
