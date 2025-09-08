use serde::{Deserialize, Serialize};

pub mod db;
pub mod handlers;
pub mod model;
pub mod routes;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateUpdateRequest {
    pub short: String,
    pub target: String,
    pub owner: Option<String>,
}

impl CreateUpdateRequest {
    fn into_with(self, original_link: model::Link) -> model::Link {
        model::Link {
            id: model::normalized_id(&self.short),
            short: self.short.clone(),
            long: self.target.clone(),
            created: original_link.created,
            updated: chrono::Utc::now(),
            owner: self.owner.clone(),
        }
    }
}

impl Into<model::Link> for CreateUpdateRequest {
    fn into(self) -> model::Link {
        model::Link {
            id: model::normalized_id(&self.short),
            short: self.short.clone(),
            long: self.target.clone(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            owner: self.owner.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LinkResponse {
    short: String,
    target: String,
    owner: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    created: chrono::DateTime<chrono::Utc>,
    #[serde(with = "chrono::serde::ts_seconds")]
    updated: chrono::DateTime<chrono::Utc>,
}

impl From<model::Link> for LinkResponse {
    fn from(value: model::Link) -> Self {
        LinkResponse {
            short: value.short,
            target: value.long,
            owner: value.owner,
            created: value.created,
            updated: value.updated,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DetailsResponse {
    pub link: model::Link,
    pub stats: Option<model::ClickStats>,
    pub csrf_string: String,
}
