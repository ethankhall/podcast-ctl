use chrono::{serde::ts_seconds, DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChannelConfig {
    #[serde(flatten)]
    pub channel: ChannelDetails,
    pub publishing: PublishingConfig,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PublishingConfig {
    pub region: Region,
    pub bucket: String,
    pub prefix: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub name: String,
    pub endpoint: String,
}

impl From<Region> for rusoto_core::Region {
    fn from(input: Region) -> Self {
        rusoto_core::Region::Custom {
            name: input.name,
            endpoint: input.endpoint,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChannelDetails {
    pub title: String,
    pub link: Option<String>,
    pub description: String,
    pub subtitle: String,
    pub summary: String,
    pub explicit: bool,
    pub image: String,
    pub owner: OwnerDetails,
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl ChannelDetails {
    #[cfg(test)]
    pub fn make_test() -> Self {
        Self {
            title: "title".to_owned(),
            link: Some("link".to_owned()),
            description: "description".to_owned(),
            subtitle: "subtitle".to_owned(),
            summary: "summary".to_owned(),
            explicit: true,
            image: "image".to_owned(),
            owner: OwnerDetails {
                name: "test".to_owned(),
                email: "email".to_owned(),
            },
            keywords: vec!["keyword".to_owned()],
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OwnerDetails {
    pub name: String,
    pub email: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub id: String,
    pub title: String,
    pub description: String,
    pub summary: String,
    pub link: Option<String>,
    pub image: String,
    #[serde(with = "ts_seconds")]
    pub released_at: DateTime<Utc>,
    pub media: EpisodeMedia,
    pub keywords: Vec<String>,
}

impl Episode {
    #[cfg(test)]
    pub fn make_test(title: &str) -> Self {
        Self {
            id: title.to_owned(),
            title: title.to_owned(),
            description: "description".to_owned(),
            summary: "summary".to_owned(),
            link: Some("link".to_owned()),
            image: "image".to_owned(),
            released_at: Utc::now(),
            media: EpisodeMedia {
                url: "url".to_owned(),
                duration: 12,
                bytes: 1000,
            },
            keywords: vec!["keyword".to_owned()],
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeMedia {
    pub url: String,
    pub duration: u64,
    pub bytes: u64,
}
