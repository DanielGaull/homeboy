use regex::Regex;
use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::time::Duration;

const SERPAPI_URL: &str = "https://serpapi.com/search";
const HUGGINGFACE_SUMMARIZATION_API: &str = "https://api-inference.huggingface.co/models/facebook/bart-large-cnn";

#[derive(Debug, Deserialize)]
struct SerpResult {
    link: String,
}

#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    organic_results: Vec<SerpResult>,
}

#[derive(Debug, Serialize)]
struct HFInput {
    inputs: String,
}

#[derive(Debug, Deserialize)]
struct HFOutput {
    summary_text: String,
}

pub struct WebSummarizer {
    client: Client,
    serp_api_key: String,
    hf_token: String,
}

#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("HuggingFace API error: {0}")]
    HuggingFaceApiError(String),
}

impl WebSummarizer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let serp_api_key = env::var("SERP_API_KEY")?;
        let hf_token = env::var("HF_API_TOKEN")?;

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            serp_api_key,
            hf_token,
        })
    }

    async fn search_google(&self, query: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let res = self
            .client
            .get(SERPAPI_URL)
            .query(&[
                ("q", query),
                ("api_key", &self.serp_api_key),
                ("engine", "google"),
                ("num", "5"),
            ])
            .send().await?
            .json::<SerpApiResponse>().await?;

        Ok(res.organic_results.into_iter().map(|r| r.link).collect())
        // Ok(vec![String::from("https://www.rose-hulman.edu/index.html")])
    }

    fn extract_text_from_html(&self, html: &str) -> String {
        let document = Html::parse_document(html);
        let body_selector = Selector::parse("body").unwrap();
        let skip_selector = Selector::parse(
            "script, style, noscript, aside, nav, header, footer, form, input, button, svg, canvas, iframe, object, embed, video, audio, picture, figure, link, meta"
        ).unwrap();
    
        let mut text = String::new();
    
        for body in document.select(&body_selector) {
            for element in body.children().filter_map(ElementRef::wrap) {
                if skip_selector.matches(&element) {
                    continue;
                }
    
                let raw_text = element.text().collect::<Vec<_>>().join(" ");
                let clean = Regex::new(r"\s+").unwrap().replace_all(&raw_text, " ");
                text.push_str(&clean);
                text.push(' ');
            }
        }
    
        text.trim().to_string()
    }

    async fn summarize_text(&self, text: &str) -> Result<String, Box<dyn Error>> {
        let input = HFInput {
            inputs: text.to_string(),
        };

        let res = self
            .client
            .post(HUGGINGFACE_SUMMARIZATION_API)
            .bearer_auth(&self.hf_token)
            .header("Content-Type", "application/json")
            .json(&input)
            .send().await?;

        if res.status().is_success() {
            let json: Vec<HFOutput> = res.json().await?;
            Ok(json.get(0).map(|o| o.summary_text.clone()).unwrap_or_default())
        } else {
            Err(Box::new(SearchError::HuggingFaceApiError(
                format!("HuggingFace API error: {} {}", res.status(), res.text().await?)
            )))
        }
    }

    pub async fn summarize_topic(&self, query: &str) -> Result<String, Box<dyn Error>> {
        let urls = self.search_google(query).await?;

        let mut all_text = String::new();

        println!("URLS: {:?}", urls);

        for url in urls {
            match self.client.get(&url).send().await {
                Ok(res) => {
                    if let Ok(html) = res.text().await {
                        println!("\n\nText ({}):\n {}\n\n", url, html);
                        let text = self.extract_text_from_html(&html);
                        if !text.is_empty() {
                            all_text.push_str(&text);
                            all_text.push_str("\n\n");
                        }
                    }
                }
                Err(err) => return Err(Box::new(err)),
            }
        }

        let input_chunk: String = all_text.chars().take(3000).collect();

        self.summarize_text(&input_chunk).await
    }
}
