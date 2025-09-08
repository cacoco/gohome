use std::{collections::HashMap, convert::Infallible, sync::OnceLock};

use chrono::Utc;
use handlebars::{Handlebars, handlebars_helper};
use regex::Regex;

use crate::{CreateUpdateRequest, DetailsResponse, LinkResponse, db, model};

static HANDLEBARS_REGISTRY: OnceLock<Handlebars<'static>> = OnceLock::new();
handlebars_helper!(query_escape: |query: String| urlencoding::encode(&query).to_owned());
handlebars_helper!(path_escape: |path: String| urlencoding::encode(&path).to_owned());
handlebars_helper!(trim_suffix: |path: String, suffix: String| {
    match path.strip_suffix(&suffix) {
        Some(result) => result,
        _ => &path
    }.to_string()
});
handlebars_helper!(trim_prefix: |path: String, prefix: String| {
    match path.strip_prefix(&prefix) {
        Some(result) => result,
        _ => &path
    }.to_string()
});
handlebars_helper!(to_lower: |s: String| s.to_lowercase());
handlebars_helper!(to_upper: |s: String| s.to_uppercase());
handlebars_helper!(now: |*kwargs| {
    tracing::trace!("{:?}", kwargs);
    Utc::now().to_rfc3339()
});
handlebars_helper!(match_string: |pattern: String, path: String| {
    let re: Result<Regex, _> = pattern.try_into();
    match re {
        Ok(r) => {
            r.is_match(&path)
        },
        _ => false
    }
});

fn setup() -> Handlebars<'static> {
    let mut bars: Handlebars<'static> = Handlebars::new();
    bars.register_helper("QueryEscape", Box::new(query_escape));
    bars.register_helper("PathEscape", Box::new(path_escape));
    bars.register_helper("ToLower", Box::new(to_lower));
    bars.register_helper("ToUpper", Box::new(to_upper));
    bars.register_helper("TrimSuffix", Box::new(trim_suffix));
    bars.register_helper("TrimPrefix", Box::new(trim_prefix));
    bars.register_helper("Now", Box::new(now));
    bars.register_helper("Match", Box::new(match_string));
    bars
}

pub async fn detail(short: &str, db: db::Db) -> Result<Box<dyn warp::Reply>, Infallible> {
    let link_id = model::normalized_id(&short);
    tracing::debug!("normalized link id: {}", &link_id);

    match db.link.get(&link_id).await {
        Ok(link) => {
            let owned_link = link.clone();
            match db.stats.get(&link.id).await {
                Ok(stats) => {
                    let details = DetailsResponse {
                        link: owned_link,
                        stats,
                        csrf_string: db.csrf_token.b64_string(),
                    };
                    Ok(Box::new(warp::reply::json(&details)))
                }
                Err(e) => {
                    tracing::error!("{e}");
                    let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                    Ok(Box::new(reply))
                }
            }
        }
        Err(e) => {
            tracing::error!("{e}");
            let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::NOT_FOUND);
            Ok(Box::new(reply))
        }
    }
}

pub async fn all(db: db::Db) -> Result<Box<dyn warp::Reply>, Infallible> {
    match db.link.get_all().await {
        Ok(links) => Ok(Box::new(warp::reply::json(&links))),
        Err(e) => {
            tracing::error!("{e}");
            let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
            Ok(Box::new(reply))
        }
    }
}

pub async fn create(request: CreateUpdateRequest, db: db::Db) -> Result<Box<dyn warp::Reply>, Infallible> {
    let links = db.link.get_all().await.unwrap();
    let link_id = model::normalized_id(&request.short);
    for link in links.iter() {
        if link_id == *link.0 {
            return Ok(Box::new(warp::http::StatusCode::BAD_REQUEST));
        }
    }

    let link: model::Link = request.into();
    tracing::debug!("creating new link: {:#?}", &link);
    match db.link.insert(&link).await {
        Ok(id) => {
            tracing::trace!("saved new db entry with id: {}", id);
            match db.link.get(&id).await {
                Ok(saved_link) => {
                    let response: LinkResponse = saved_link.into();
                    let reply = warp::reply::with_header(
                        warp::reply::with_status(warp::reply::json(&response), warp::http::StatusCode::CREATED),
                        "Location",
                        format!("/{}", &response.short),
                    );
                    Ok(Box::new(reply))
                }
                Err(e) => {
                    tracing::error!("{e}");
                    let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                    Ok(Box::new(reply))
                }
            }
        }
        Err(e) => {
            tracing::error!("{e}");
            let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
            Ok(Box::new(reply))
        }
    }
}

pub async fn update(id: &str, request: CreateUpdateRequest, db: db::Db) -> Result<Box<dyn warp::Reply>, Infallible> {
    let links = db.link.get_all().await.unwrap();
    tracing::debug!("current links count: {}", &links.len());

    match db.link.get(id).await {
        Ok(link) => {
            let updated_link: model::Link = request.into_with(link);
            tracing::debug!("updating link id: {id}");
            match db.link.update(&updated_link).await {
                Ok(()) => {
                    tracing::trace!("saved new db entry with id: {}", id);
                    let response: LinkResponse = updated_link.into();
                    let reply = warp::reply::with_header(
                        warp::reply::with_status(warp::reply::json(&response), warp::http::StatusCode::OK),
                        "Location",
                        format!("/{}", &response.short),
                    );
                    Ok(Box::new(reply))
                }
                Err(e) => {
                    tracing::error!("{e}");
                    let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                    Ok(Box::new(reply))
                }
            }
        }
        Err(e) => {
            tracing::error!("{e}");
            let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::NOT_FOUND);
            Ok(Box::new(reply))
        }
    }
}

pub async fn get(
    short: &str,
    full_path: &str,
    query_params: HashMap<String, String>,
    db: db::Db,
) -> Result<Box<dyn warp::Reply>, Infallible> {
    tracing::info!("full path: {}", full_path);
    tracing::info!("query params: {:#?}", query_params);

    let link_id = model::normalized_id(short);
    let link = db.link.get(&link_id).await;
    match link {
        Ok(link) => match expand_link(full_path, query_params, &link.long) {
            Ok(location) => Ok(Box::new(warp::redirect(location.parse::<warp::http::Uri>().unwrap()))),
            Err(e) => {
                tracing::error!("{}", e);
                let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                Ok(Box::new(reply))
            }
        },
        Err(e) => {
            tracing::error!("{}", e);
            let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::NOT_FOUND);
            Ok(Box::new(reply))
        }
    }
}

fn expand_link(
    full_path: &str,
    query_params: HashMap<String, String>,
    long: &str,
) -> Result<String, handlebars::RenderError> {
    let mut path_tokens: Vec<&str> = full_path.split("/").collect();

    let mut path = String::new();
    if path_tokens.len() > 1 {
        path_tokens.remove(0);
        path = path_tokens.join("/");
    }

    let handlebars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
    handlebars
        .render_template(long, &serde_json::json!({"Path": path}))
        .map(|expanded| {
            if !query_params.is_empty() {
                let mut query_string_vec: Vec<String> = Vec::new();
                for (key, value) in &query_params {
                    query_string_vec.push(format!("{}={}", key, value));
                }
                let query_string = urlencoding::encode(&query_string_vec.join("&")).into_owned();
                format!("{}?{}", expanded, query_string)
            } else {
                expanded
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_escape() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "https://www.google.com/{{#if Path}}search?q={{QueryEscape Path}}{{/if}}",
                &serde_json::json!({}),
            )
            .unwrap();
        assert_eq!(res, "https://www.google.com/");
    }

    #[test]
    fn test_query_escape_1() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "https://www.google.com/{{#if Path}}search?q={{QueryEscape Path}}{{/if}}",
                &serde_json::json!({"Path": "Tolstoy"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Tolstoy");
    }

    #[test]
    fn test_query_escape_2() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "https://www.google.com/{{#if Path}}search?q={{QueryEscape Path}}{{/if}}",
                &serde_json::json!({"Path": "Foo Bar baz"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Foo%20Bar%20baz");
    }

    #[test]
    fn test_path() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "https://www.nytimes.com/{{#if Path}}{{Path}}{{/if}}",
                &serde_json::json!({"Path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025/09/05/theater/broadway.html");
    }

    #[test]
    fn test_path_escape() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "https://www.nytimes.com/{{#if Path}}{{PathEscape Path}}{{/if}}",
                &serde_json::json!({"Path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025%2F09%2F05%2Ftheater%2Fbroadway.html");
    }

    #[test]
    fn test_to_lower() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "{{#if Path}}{{ToLower Path}}{{/if}}",
                &serde_json::json!({"Path": "SAMIAM"}),
            )
            .unwrap();
        assert_eq!(res, "samiam");
    }

    #[test]
    fn test_to_upper() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "{{#if Path}}{{ToUpper Path}}{{/if}}",
                &serde_json::json!({"Path": "samiam"}),
            )
            .unwrap();
        assert_eq!(res, "SAMIAM");
    }

    #[test]
    fn test_trim_suffix() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "{{#if Path}}{{TrimSuffix Path '/'}}{{/if}}",
                &serde_json::json!({"Path": "a/"}),
            )
            .unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_trim_suffix_1() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "{{TrimSuffix Path ', world'}}",
                &serde_json::json!({"Path": "hello, world"}),
            )
            .unwrap();
        assert_eq!(res, "hello");
    }

    #[test]
    fn test_trim_suffix_2() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template("{{#if Path}}{{TrimSuffix Path '/'}}{{/if}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_prefix() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "{{#if Path}}{{TrimPrefix Path '/'}}{{/if}}",
                &serde_json::json!({"Path": "/a"}),
            )
            .unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_prefix_1() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "{{TrimPrefix Path 'hello, '}}",
                &serde_json::json!({"Path": "hello, world"}),
            )
            .unwrap();
        assert_eq!(res, "world");
    }

    #[test]
    fn test_prefix_2() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template("{{#if Path}}{{TrimPrefix Path '/'}}{{/if}}", &serde_json::json!({}))
            .unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_now() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template("{{ Now }}", &serde_json::json!({"Path": "foobar"}))
            .unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_now_1() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars.render_template("{{ Now }}", &serde_json::json!({})).unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn test_match() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                r#"http://host.com/{{#if (Match "\\d+" Path)}}id/{{Path}}{{else}}search/{{Path}}{{/if}}"#,
                &serde_json::json!({"Path": "123"}),
            )
            .unwrap();
        assert_eq!(res, "http://host.com/id/123");
    }

    #[test]
    fn test_match_1() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                r#"http://host.com/{{#if (Match "\\d+" Path)}}id/{{Path}}{{else}}search/{{Path}}{{/if}}"#,
                &serde_json::json!({"Path": "foo"}),
            )
            .unwrap();
        assert_eq!(res, "http://host.com/search/foo");
    }
}
