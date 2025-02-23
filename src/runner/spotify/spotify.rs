use std::{env, error::Error};

use rspotify::{model::{Country, Id, Market, PlayableId, SearchResult, SearchType, TrackId}, prelude::{BaseClient, OAuthClient}, scopes, AuthCodeSpotify, Credentials, OAuth};

pub struct Spotify {
    client: AuthCodeSpotify,
}

pub struct Song {
    pub id: String,
    pub name: String,
    pub artist: String,
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
        let spotify = AuthCodeSpotify::new(creds, oauth);
        let url = spotify.get_authorize_url(false).unwrap();
        spotify.prompt_for_token(&url).await?;

        Ok(Spotify {
            client: spotify
        })
    }
    
    pub async fn get_song(&self, query: String) -> Result<Option<Song>, Box<dyn Error>> {
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
                return Ok(
                    Some(
                        Song {
                            id: String::from(track.id.clone().unwrap().id()),
                            name: track.name.clone(),
                            artist: String::new(),
                        }
                    )
                );
            }
        }
        Ok(None)
    }

    pub async fn play_song(&self, id: String) -> Result<(), Box<dyn Error>> {
        self.client.start_uris_playback(vec![PlayableId::Track(TrackId::from_id(id).unwrap())], None, None, None).await?;
        Ok(())
    }
}
