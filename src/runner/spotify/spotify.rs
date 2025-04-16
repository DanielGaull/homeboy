use std::{env, error::Error};

use rspotify::{model::{Country, DeviceType, Id, Market, PlayableId, SearchResult, SearchType, TrackId}, prelude::{BaseClient, OAuthClient}, scopes, AuthCodeSpotify, Credentials, OAuth};

pub struct Spotify {
    client: Option<AuthCodeSpotify>,
}

pub struct Song {
    pub id: String,
    pub name: String,
    pub artist: String,
}

impl Spotify {
    pub fn new() -> Self {
        Spotify {
            client: None,
        }
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn Error>> {
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

        self.client = Some(spotify);
        Ok(())
    }
    
    pub async fn get_song(&self, query: String) -> Result<Option<Song>, Box<dyn Error>> {
        let result = self.client.as_ref().unwrap().search(
            &query, 
            SearchType::Track, 
            Some(Market::Country(Country::UnitedStates)), 
            None, 
            Some(10), 
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

    pub async fn play_song(&self, id: String, device_type: u8) -> Result<(), Box<dyn Error>> {
        // 0 = whatever is currently used
        // 1 = computer
        // 2 = phone
        let devices = self.client.as_ref().unwrap().device().await?;
        let mut device_to_use = None;
        if device_type != 0 {
            let type_to_find = 
                if device_type == 1 {
                    DeviceType::Computer
                } else {
                    DeviceType::Smartphone
                };
            for d in &devices {
                if d._type == type_to_find {
                    device_to_use = Some(d);
                    break;
                }
            }
        }

        if device_to_use.is_none() && devices.len() > 0 {
            device_to_use = devices.get(0);
        }

        self.client.as_ref().unwrap().start_uris_playback(
            vec![PlayableId::Track(TrackId::from_id(id).unwrap())],
            device_to_use.map(|f| f.id.clone()).flatten().as_deref(),
            None,
            None,
        ).await?;
        Ok(())
    }

    pub async fn pause(&self) -> Result<(), Box<dyn Error>> {
        self.client.as_ref().unwrap().pause_playback(None).await?;
        Ok(())
    }

    pub async fn resume(&self) -> Result<(), Box<dyn Error>> {
        self.client.as_ref().unwrap().resume_playback(None, None).await?;
        Ok(())
    }

    pub async fn skip(&self) -> Result<(), Box<dyn Error>> {
        self.client.as_ref().unwrap().next_track(None).await?;
        Ok(())
    }

    pub async fn queue_song(&self, id: String, device_type: u8) -> Result<(), Box<dyn Error>> {
        // 0 = whatever is currently used
        // 1 = computer
        // 2 = phone
        let devices = self.client.as_ref().unwrap().device().await?;
        let mut device_to_use = None;
        if device_type != 0 {
            let type_to_find = 
                if device_type == 1 {
                    DeviceType::Computer
                } else {
                    DeviceType::Smartphone
                };
            for d in &devices {
                if d._type == type_to_find {
                    device_to_use = Some(d);
                    break;
                }
            }
        }

        if device_to_use.is_none() && devices.len() > 0 {
            device_to_use = devices.get(0);
        }

        self.client.as_ref().unwrap().add_item_to_queue(
            PlayableId::Track(TrackId::from_id(id).unwrap()),
            device_to_use.map(|f| f.id.clone()).flatten().as_deref()
        ).await?;
        Ok(())
    }
}
