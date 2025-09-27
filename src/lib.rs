use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
            id: Uuid::new_v4(),
            short: val.short.clone(),
            long: val.target.clone(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        }
    }
}
