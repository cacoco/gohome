use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod db;
pub mod render;
pub mod model;
pub mod routes;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateUpdateRequest {
    pub short: String,
    pub target: String,
}

impl Into<model::Link> for CreateUpdateRequest {
    fn into(self) -> model::Link {
        model::Link {
            id: Uuid::new_v4(),
            short: self.short.clone(),
            long: self.target.clone(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        }
    }
}
