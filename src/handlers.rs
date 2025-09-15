use std::{collections::HashMap, convert::Infallible, sync::OnceLock};

use chrono::Utc;
use handlebars::{Handlebars, handlebars_helper};
// use maud::html;
use regex::Regex;

use crate::{db, model, AllResponse, CreateUpdateRequest, DetailsResponse, LinkResponse};

static HANDLEBARS_REGISTRY: OnceLock<Handlebars<'static>> = OnceLock::new();
handlebars_helper!(encode: |query: String| urlencoding::encode(&query).to_owned());
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

handlebars_helper!(now_format: |format: String| {
    Utc::now().format(&format).to_string()
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
    bars.register_helper("encode", Box::new(encode));
    bars.register_helper("lowercase", Box::new(to_lower));
    bars.register_helper("uppercase", Box::new(to_upper));
    bars.register_helper("trimsuffix", Box::new(trim_suffix));
    bars.register_helper("trimprefix", Box::new(trim_prefix));
    bars.register_helper("now", Box::new(now));
    bars.register_helper("nowformat", Box::new(now_format));
    bars.register_helper("match", Box::new(match_string));
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
        Ok(links) => {
            let response = AllResponse {
                links: links.into_values().collect(),
                csrf_string: db.csrf_token.b64_string(),
            };
            Ok(Box::new(warp::reply::json(&response)))
        },
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
                    let response: LinkResponse = saved_link.into_with(db.csrf_token.b64_string());
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
            let updated_link: model::Link = model::Link {
                id: link.id,
                short: link.short,
                long: request.target,
                created: link.created,
                updated: chrono::Utc::now(),
                owner: request.owner,
            };
            tracing::debug!("updating link id: {id}");
            match db.link.update(&updated_link).await {
                Ok(()) => {
                    tracing::trace!("saved new db entry with id: {}", id);
                    let response: LinkResponse = updated_link.into_with(db.csrf_token.b64_string());
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
        Ok(link) => {
            let path = path_remainder(full_path);
            match expand_link(&path, query_params, &link.long) {
                Ok(location) => Ok(Box::new(warp::redirect(location.parse::<warp::http::Uri>().unwrap()))),
                Err(e) => {
                    tracing::error!("{}", e);
                    let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                    Ok(Box::new(reply))
                }
            }
        },
        Err(e) => {
            tracing::error!("{}", e);
            let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::NOT_FOUND);
            Ok(Box::new(reply))
        }
    }
}

// returns the remaining path after the short identifier, e.g.,
// go/foo/bar --> bar
// go/github/repo/a --> repo/a
fn path_remainder(full_path: &str) -> String {
    let mut path_tokens: Vec<&str> = full_path.split("/").collect();

    let mut path = String::new();
    if path_tokens.len() > 1 {
        path_tokens.remove(0);
        path = path_tokens.join("/");
    }
    path
}

fn expand_link(
    path: &str,
    query_params: HashMap<String, String>,
    long: &str,
) -> Result<String, handlebars::RenderError> {
    let handlebars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
    let template = 
        if !long.contains("{{") && !path.is_empty() {
            if long.ends_with("/") {
                format!("{}{}", long, "{{path}}")
            } else {
                format!("{}/{}", long, "{{path}}")
            }
        } else {
            long.to_string()
        };
    handlebars
        .render_template(&template, &serde_json::json!({"path": path}))
        .map(|expanded| {
            if !query_params.is_empty() {
                let mut query_string_vec: Vec<String> = Vec::new();
                for (key, value) in &query_params {
                    query_string_vec.push(format!("{}={}", urlencoding::encode(key), urlencoding::encode(value)));
                }
                let query_string = &query_string_vec.join("&");
                if expanded.contains("?") {
                    format!("{}&{}", expanded, query_string)
                } else {
                    format!("{}?{}", expanded, query_string)
                }
            } else {
                expanded
            }
        })
}

#[cfg(test)]
mod tests {
    use std::{any::Any, borrow::Cow};

    use url::Url;

    use super::*;

    #[test]
    fn test_encode() {
        let res = expand_link("", HashMap::new(), "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
        assert_eq!(res, "https://www.google.com/");
    }

    #[test]
    fn test_encode_1() {
        let res = expand_link("Tolstoy", HashMap::new(), "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Tolstoy");
    }

    #[test]
    fn test_encode_2() {
        let res = expand_link("Foo Bar baz", HashMap::new(), "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Foo%20Bar%20baz");
    }

    #[test]
    fn test_with_query_string() {
        let mut query_params = HashMap::new();
        query_params.insert("a".to_string(), "1".to_string());
        query_params.insert("bb".to_string(), "2".to_string());
        let res = expand_link("Foo Bar baz", query_params, "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
        let url = Url::parse(&res).unwrap();
        let pairs = url.query_pairs();
        assert!(pairs.count() == 3);

        let mut query_map: HashMap<String, String> = HashMap::new();
        for pair in pairs {
            query_map.insert(pair.0.to_string(), pair.1.to_string());
        }
        assert_eq!(*query_map.get("q").unwrap(), String::from("Foo Bar baz"));
        assert_eq!(*query_map.get("a").unwrap(), String::from("1"));
        assert_eq!(*query_map.get("bb").unwrap(), String::from("2"));
    }

    #[test]
    fn test_path() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "https://www.nytimes.com/{{#if path}}{{path}}{{/if}}",
                &serde_json::json!({"path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025/09/05/theater/broadway.html");
    }

    #[test]
    fn test_path_escape() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "https://www.nytimes.com/{{#if path}}{{encode path}}{{/if}}",
                &serde_json::json!({"path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025%2F09%2F05%2Ftheater%2Fbroadway.html");
    }

    #[test]
    fn test_to_lower() {
        let bars = HANDLEBARS_REGISTRY.get_or_init(|| setup());
        let res = bars
            .render_template(
                "{{#if path}}{{lowercase path}}{{/if}}",
                &serde_json::json!({"path": "SAMIAM"}),
            )
            .unwrap();
        assert_eq!(res, "samiam");
    }

    #[test]
    fn test_to_upper() {
        let res = expand_link("samiam", HashMap::new(), "{{#if path}}{{uppercase path}}{{/if}}").unwrap();
        assert_eq!(res, "SAMIAM");
    }

    #[test]
    fn test_trim_suffix() {
        let res = expand_link("a/", HashMap::new(), "{{#if path}}{{trimsuffix path '/'}}{{/if}}").unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_trim_suffix_1() {
        let res = expand_link("hello, world", HashMap::new(), "{{trimsuffix path ', world'}}").unwrap();
        assert_eq!(res, "hello");
    }

    #[test]
    fn test_trim_suffix_2() {
        let res = expand_link("", HashMap::new(), "{{#if path}}{{trimsuffix path '/'}}{{/if}}").unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_prefix() {
        let res = expand_link("OOOa", HashMap::new(),"{{#if path}}{{trimprefix path 'OOO'}}{{/if}}").unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_prefix_1() {
        let res = expand_link("hello, world", HashMap::new(), "{{trimprefix path 'hello, '}}").unwrap();
        assert_eq!(res, "world");
    }

    #[test]
    fn test_prefix_2() {
        let res = expand_link("", HashMap::new(), "{{#if path}}{{trimprefix path '/'}}{{/if}}").unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_now_with_path() {
        let res = expand_link("foobar", HashMap::new(), "{{ now }}").unwrap();
        assert!(!res.is_empty());
        // res should just be the date -- no path in template
        let parsed: chrono::DateTime<Utc> = res.parse().unwrap();
        assert!(parsed.type_id() == std::any::TypeId::of::<chrono::DateTime<Utc>>());
    }

    #[test]
    fn test_now_no_path() {
        let res = expand_link("", HashMap::new(), "{{ now }}").unwrap();
        assert!(!res.is_empty());
        // res should just be the date -- no path in template
        let parsed: chrono::DateTime<Utc> = res.parse().unwrap();
        assert!(parsed.type_id() == std::any::TypeId::of::<chrono::DateTime<Utc>>());
    }

    #[test]
    fn test_match() {
        let res = expand_link("123", HashMap::new(),r#"http://host.com/{{#if (match "\\d+" path)}}id/{{path}}{{else}}search/{{path}}{{/if}}"#).unwrap();
        assert_eq!(res, "http://host.com/id/123");
    }

    #[test]
    fn test_match_1() {
        let res = expand_link("foo", HashMap::new(), r#"http://host.com/{{#if (match "\\d+" path)}}id/{{path}}{{else}}search/{{path}}{{/if}}"#).unwrap();
        assert_eq!(res, "http://host.com/search/foo");
    }

    #[test]
    fn test_no_mangle_escapes() {
        let res = expand_link("", HashMap::new(), "http://host.com/foo%2f/bar").unwrap();
        assert_eq!(res, "http://host.com/foo%2f/bar");
    }

    #[test]
    fn test_no_mangle_escapes_with_path() {
        let res = expand_link("extra", HashMap::new(), "http://host.com/foo%2f/bar").unwrap();
        assert_eq!(res, "http://host.com/foo%2f/bar/extra");
    }

    #[test]
    fn test_remainder() {
        let res = expand_link("extra", HashMap::new(), "http://host.com/foo").unwrap();
        assert_eq!(res, "http://host.com/foo/extra");
    }

    #[test]
    fn test_remainder_with_slash() {
        let res = expand_link("extra", HashMap::new(), "http://host.com/foo/").unwrap();
        assert_eq!(res, "http://host.com/foo/extra");
    }

    #[test]
    fn test_now_format() {
        let res = expand_link("",HashMap::new(), r#"https://roamresearch.com/#/app/ts-corp/page/{{ nowformat "%d/%m/%Y"}}"#).unwrap();
        let path = res.strip_prefix("https://").unwrap();
        let segments: Vec<&str> = path.split("/").collect();
        assert!(segments.len() == 8); // roamresearch.com + # + app + ts-corp + page + d + m + Y = 8
    }

    #[test]
    fn test_undefined_field() {
        let res = expand_link("bar", HashMap::new(), "http://host.com/{{ bar }}").unwrap();
        assert_eq!(res, "http://host.com/");
    }

    #[test]
    fn test_defined_field() {
        let res = expand_link("bar", HashMap::new(), "http://host.com/{{path}}").unwrap();
        assert_eq!(res, "http://host.com/bar");
    }
}
