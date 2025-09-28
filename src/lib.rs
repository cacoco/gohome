use serde::{Deserialize, Serialize};

pub mod db;
pub mod model;
pub mod render;
pub mod routes;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateUpdateRequest {
    pub short: String,
    pub target: String,
}

impl From<CreateUpdateRequest> for model::Link {
    fn from(val: CreateUpdateRequest) -> Self {
        model::Link {
            short: val.short.clone(),
            long: val.target.clone(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        }
    }
}
