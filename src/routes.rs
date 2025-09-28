use std::{collections::HashMap, convert::Infallible};

use uuid::Uuid;
use warp::{Filter, filters::path::FullPath};

use crate::{CreateUpdateRequest, render::Renderer};

// If the caller sends this header set to a non-empty value, we will allow
// them to make the call even without an XSRF token. JavaScript in browser
// cannot set this header, per the [Fetch Spec].
//
// [Fetch Spec]: https://fetch.spec.whatwg.org
const SEC_HEADER_NAME: &str = "Sec-Golink";

fn with_renderer(handlers: Renderer) -> impl Filter<Extract = (Renderer,), Error = Infallible> + Clone {
    warp::any().map(move || handlers.clone())
}

fn home(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path::end()
        .and(warp::get())
        .and(with_renderer(renderer))
        .and_then(|renderer: Renderer| async move { renderer.home().await })
}

fn all(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".all")
        .and(warp::get())
        .and(with_renderer(renderer))
        .and_then(|renderer: Renderer| async move { renderer.all().await })
}

fn detail(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".detail" / String)
        .and(warp::get())
        .and(with_renderer(renderer))
        .and_then(|short: String, renderer: Renderer| async move { renderer.detail(&short).await })
}

fn create(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".create")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
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
    warp::path(".update")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::form())
        .and(with_renderer(renderer))
        .and_then(|form_data: HashMap<String, String>, renderer: Renderer| async move {
            let xsrf = form_data.get("xsrf").unwrap().to_string();
            let id = Uuid::parse_str(form_data.get("id").unwrap()).expect("Unable to parse UUIDv4");
            let request = CreateUpdateRequest {
                short: form_data.get("short").unwrap().to_string(),
                target: form_data.get("long").unwrap().to_string(),
            };
            renderer.update(&id, request, &xsrf).await
        })
}

fn delete(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!(".delete" / String)
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::form())
        .and(with_renderer(renderer))
        .and_then(
            |id_string: String, form_data: HashMap<String, String>, renderer: Renderer| async move {
                let xsrf = form_data.get("xsrf").unwrap().to_string();
                let id = Uuid::parse_str(&id_string).expect("Unable to parse UUIDv4");
                renderer.delete(&id, &xsrf).await
            },
        )
}

fn help(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".help")
        .and(warp::get())
        .and(with_renderer(renderer))
        .and_then(|renderer: Renderer| async move { renderer.help().await })
}

fn export(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path(".export")
        .and(warp::get())
        .and(with_renderer(renderer))
        .and_then(|renderer: Renderer| async move { renderer.export().await })
}

fn get(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::get()
        .and(warp::path::param::<String>())
        .and(warp::path::full())
        .and(warp::query::<HashMap<String, String>>())
        .and(with_renderer(renderer))
        .and_then(|short: String, path: FullPath, query_params: HashMap<String, String>, renderer: Renderer| async move {
            let path_as_str = path.as_str();
            if path_as_str.ends_with("+") {
                let trimmed = short.strip_suffix("+").unwrap_or(path_as_str);
                renderer.json_detail(trimmed).await
            } else {
                renderer.get(&short, path.as_str(), query_params).await
            }
        })
}

fn post(renderer: Renderer) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path::end()
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::form())
        .and(warp::header::<String>(SEC_HEADER_NAME))
        .and(with_renderer(renderer))
        .and_then(
            |form_data: HashMap<String, String>, sec_header_value: String, renderer: Renderer| async move {
                if sec_header_value.is_empty() {
                    renderer.bad_request().await
                } else {
                    let request = CreateUpdateRequest {
                        short: form_data.get("short").unwrap().to_string(),
                        target: form_data.get("long").unwrap().to_string(),
                    };
                    renderer.new_link(request).await
                }
            },
        )
}

pub fn get_routes(
    renderer: Renderer,
    assets: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let routes = post(renderer.clone())
        .or(detail(renderer.clone()))
        .or(all(renderer.clone()))
        .or(help(renderer.clone()))
        .or(export(renderer.clone()))
        .or(get(renderer.clone()))
        .or(home(renderer.clone()))
        .or(create(renderer.clone()))
        .or(update(renderer.clone()))
        .or(delete(renderer.clone()));

    let static_route = warp::path("assets").and(warp::fs::dir(assets));
    static_route.or(routes)
}
