use std::{collections::HashMap, convert::Infallible};

use warp::{Filter, filters::path::FullPath};

use crate::{CreateUpdateRequest, db::Db, handlers};

fn with_db(db: Db) -> impl Filter<Extract = (Db,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}

fn json_body() -> impl Filter<Extract = (CreateUpdateRequest,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

fn all(db: Db) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".all").and(with_db(db)).and_then(handlers::all)
}

fn detail(db: Db) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".detail" / String)
        .and(with_db(db))
        .and_then(|short: String, db: Db| async move { handlers::detail(&short, db).await })
}

fn create(db: Db) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::post()
        .and(warp::body::content_length_limit(1024 * 16))
        .and(json_body())
        .and(with_db(db))
        .and_then(handlers::create)
}

fn update(db: Db) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::patch()
        .and(warp::path::param::<String>())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(json_body())
        .and(with_db(db))
        .and_then(
            |id: String, request: CreateUpdateRequest, db: Db| async move { handlers::update(&id, request, db).await },
        )
}

fn get(db: Db) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::full())
        .and(warp::query::<HashMap<String, String>>())
        .and(with_db(db))
        .and_then(
            |short: String, path: FullPath, query_params: HashMap<String, String>, db: Db| async move {
                handlers::get(&short, path.as_str(), query_params, db).await
            },
        )
}

pub fn get_routes(db: Db) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    all(db.clone())
        .or(detail(db.clone()))
        .or(create(db.clone()))
        .or(update(db.clone()))
        .or(get(db.clone()))
}
