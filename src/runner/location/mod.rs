use std::error::Error;
use reqwest;
use serde::Deserialize;

#[derive(Deserialize)]
struct IpResponse {
    origin: String,
}

pub struct Location {
    pub lat: f32,
    pub long: f32,
    pub city: String,
}

pub async fn get_loc() -> Result<Location, Box<dyn Error>> {
    let ip: IpResponse = reqwest::get("https://httpbin.org/ip").await?.json().await?;
    let info = geolocation::find(&ip.origin)?;
    Ok(Location {
        lat: info.latitude.parse()?,
        long: info.longitude.parse()?,
        city: info.city,
    })
}
