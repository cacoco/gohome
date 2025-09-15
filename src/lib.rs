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
    created: chrono::DateTime<chrono::Utc>,
    updated: chrono::DateTime<chrono::Utc>,
    csrf_string: String,
}

impl model::Link {
    fn into_with(&self, csrf_string: String) -> LinkResponse {
        LinkResponse {
            short: self.short.clone(),
            target: self.long.clone(),
            owner: self.owner.clone(),
            created: self.created,
            updated: self.updated,
            csrf_string,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DetailsResponse {
    pub link: model::Link,
    pub stats: Option<model::ClickStats>,
    pub csrf_string: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AllResponse {
    pub links: Vec<model::Link>,
    pub csrf_string: String,
}
