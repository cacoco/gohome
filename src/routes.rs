use std::{collections::HashMap, convert::Infallible};

use uuid::Uuid;
use warp::{filters::path::FullPath, Filter};

use crate::{CreateUpdateRequest, render::Renderer};

fn with_renderer(handlers: Renderer) -> impl Filter<Extract = (Renderer,), Error = Infallible> + Clone {
    warp::any().map(move || handlers.clone())
}

fn all(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".all")
        .and(with_renderer(renderer))
        .and_then(|renderer: Renderer| async move {
            renderer.all().await
        })
}

fn detail(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".detail" / String)
        .and(with_renderer(renderer))
        .and_then(|short: String, renderer: Renderer| async move { 
            renderer.detail(&short).await 
        })
}

fn create(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".create")
        .and(warp::post())
        .and(warp::body::form())
        .and(with_renderer(renderer))
        .and_then(|form_data: HashMap<String, String>, renderer: Renderer| async move {
            let xsrf = form_data.get("xsrf").unwrap().to_string();
            let request = CreateUpdateRequest { 
                short: form_data.get("short").unwrap().to_string(),
                target: form_data.get("long").unwrap().to_string(),
            };
            renderer.create(request, &xsrf).await
        })
}

fn update(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".update")
        .and(warp::post())
        .and(warp::body::form())
        .and(with_renderer(renderer))
        .and_then(|form_data: HashMap<String, String>, renderer: Renderer| async move {
            let xsrf = form_data.get("xsrf").unwrap().to_string();
            let id = Uuid::parse_str(form_data.get("id").unwrap()).expect("Unable to parse UUIDv4");
            let request = CreateUpdateRequest { 
                short: form_data.get("short").unwrap().to_string(),
                target: form_data.get("long").unwrap().to_string(),
            };
            tracing::debug!("{request:?}");
            renderer.update(&id, request, &xsrf).await 
        })
}

fn home(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path::end()
        .and(with_renderer(renderer))
        .and_then(|renderer: Renderer| async move {
            renderer.home().await
        })
}

fn get(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::full())
        .and(warp::query::<HashMap<String, String>>())
        .and(with_renderer(renderer))
        .and_then(
            |short: String, path: FullPath, query_params: HashMap<String, String>, renderer: Renderer| async move {
                renderer.get(&short, path.as_str(), query_params).await
            }
        )
}

fn delete(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".delete" / String)
        .and(warp::post())
        .and(warp::body::form())
        .and(with_renderer(renderer))
        .and_then(|id_string: String, form_data: HashMap<String, String>, renderer: Renderer| async move {
            let xsrf = form_data.get("xsrf").unwrap().to_string();
            let id = Uuid::parse_str(&id_string).expect("Unable to parse UUIDv4");
            tracing::debug!("{id}");
            renderer.delete(&id, &xsrf).await
        })
}

fn export(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".export")
        .and(with_renderer(renderer))
        .and_then(|renderer: Renderer| async move {
            renderer.export().await
        })
}

pub fn get_routes(renderer: Renderer, static_assets: String) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let api_routes = home(renderer.clone())
        .or(all(renderer.clone()))
        .or(detail(renderer.clone()))
        .or(create(renderer.clone()))
        .or(update(renderer.clone()))
        .or(delete(renderer.clone()))
        .or(export(renderer.clone()))
        .or(get(renderer.clone()));

    let static_route = 
        warp::path("assets")
            .and(warp::fs::dir(static_assets));
    static_route.or(api_routes)
}
