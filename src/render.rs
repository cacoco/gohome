use std::{
    collections::HashMap,
    convert::Infallible,
    io::{BufWriter, Write},
};

use chrono::{DateTime, Utc};
use csrf::{AesGcmCsrfProtection, CsrfProtection};
use handlebars::Handlebars;
use rand::RngCore;
use regex::Regex;
use uuid::Uuid;

use crate::{
    CreateUpdateRequest, db,
    model::{self, PopularLink},
};

const PARENT_PARTIAL: &str = "base";

struct Message {
    msg: String,
}

impl Message {
    fn new(message: &str) -> Self {
        Self {
            msg: message.to_string(),
        }
    }
}

impl warp::Reply for Message {
    fn into_response(self) -> warp::reply::Response {
        warp::reply::Response::new(self.msg.to_string().into())
    }
}

handlebars::handlebars_helper!(encode: |query: String| urlencoding::encode(&query).clone());
handlebars::handlebars_helper!(trim_suffix: |path: String, suffix: String| {
    match path.strip_suffix(&suffix) {
        Some(result) => result,
        _ => &path
    }.to_string()
});
handlebars::handlebars_helper!(trim_prefix: |path: String, prefix: String| {
    match path.strip_prefix(&prefix) {
        Some(result) => result,
        _ => &path
    }.to_string()
});
handlebars::handlebars_helper!(to_lower: |s: String| s.to_lowercase());
handlebars::handlebars_helper!(to_upper: |s: String| s.to_uppercase());
handlebars::handlebars_helper!(now: |*_kwargs| {
    Utc::now().to_rfc3339()
});
handlebars::handlebars_helper!(now_format: |format: String| {
    Utc::now().format(&format).to_string()
});
handlebars::handlebars_helper!(date_format: |t: DateTime<Utc>, format: String| {
    t.format(&format).to_string()
});
handlebars::handlebars_helper!(match_string: |pattern: String, path: String| {
    let re: Result<Regex, _> = pattern.try_into();
    match re {
        Ok(r) => {
            r.is_match(&path)
        },
        _ => false
    }
});

#[derive(Clone, Debug)]
pub struct Renderer {
    host: String,
    csrf_token: csrf::CsrfToken,
    pub(crate) db: db::Db,
    pub(crate) handlebars: handlebars::Handlebars<'static>,
}

impl Renderer {
    pub fn empty() -> Self {
        Self::new("go", db::Db::in_memory().unwrap(), Handlebars::new())
    }

    pub fn new(host: &str, db: db::Db, handlebars: handlebars::Handlebars<'static>) -> Self {
        let mut secret_key = [0u8; 32];
        rand::rng().fill_bytes(&mut secret_key);
        let protect = AesGcmCsrfProtection::from_key(secret_key);

        let mut nonce = [0u8; 64];
        rand::rng().fill_bytes(&mut nonce);
        let csrf_token: csrf::CsrfToken = protect.generate_token(&nonce).unwrap();

        let mut bars = handlebars.clone();
        bars.register_helper("encode", Box::new(encode));
        bars.register_helper("lowercase", Box::new(to_lower));
        bars.register_helper("uppercase", Box::new(to_upper));
        bars.register_helper("trimsuffix", Box::new(trim_suffix));
        bars.register_helper("trimprefix", Box::new(trim_prefix));
        bars.register_helper("now", Box::new(now));
        bars.register_helper("nowformat", Box::new(now_format));
        bars.register_helper("dateformat", Box::new(date_format));
        bars.register_helper("match", Box::new(match_string));
        Self {
            host: host.to_string(),
            csrf_token,
            db,
            handlebars: bars,
        }
    }

    pub fn xsrf(&self) -> String {
        self.csrf_token.b64_string()
    }
}

fn redirect(location: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
    Ok(Box::new(warp::reply::with_header(
        warp::redirect(location.parse::<warp::http::Uri>().unwrap()),
        "Cache-Control",
        "no-cache",
    )))
}

fn response(message: &str, status: warp::http::StatusCode) -> Result<Box<dyn warp::Reply>, Infallible> {
    Ok(Box::new(warp::reply::with_status(Message::new(message), status)))
}

impl Renderer {
    pub async fn home(&self) -> Result<Box<dyn warp::Reply>, Infallible> {
        let mut links: Vec<(model::Link, model::ClickStats)> = Vec::new();
        match self.db.link.most_popular().await {
            Ok(mut results) => {
                links.append(&mut results);
            }
            Err(e) => {
                tracing::error!("{e}");
            }
        }

        let most_popular_links: Vec<model::PopularLink> = links
            .iter()
            .map(|(link, stats)| PopularLink {
                id: link.id.to_string(),
                short: link.short.clone(),
                clicks: stats.clicks.or(Some(0)),
            })
            .collect();
        match self.handlebars.render(
            "home",
            &serde_json::json!({"go": self.host, "parent": PARENT_PARTIAL, "links": most_popular_links, "XSRF": self.xsrf()}),
        ) {
            Ok(response) => Ok(Box::new(warp::reply::html(response))),
            Err(e) => {
                tracing::error!("{e}");
                redirect("/")
            }
        }
    }

    pub async fn detail(&self, short: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        match self.db.link.get(short).await {
            Ok(link) => {
                match self.handlebars.render(
                    "detail",
                    &serde_json::json!({"go": self.host, "parent": PARENT_PARTIAL, "link": link, "XSRF": self.xsrf()}),
                ) {
                    Ok(response) => Ok(Box::new(warp::reply::html(response))),
                    Err(e) => {
                        tracing::error!("{e}");
                        redirect("/")
                    }
                }
            }
            Err(e) => {
                tracing::error!("{e}");
                redirect("/")
            }
        }
    }

    pub async fn all(&self) -> Result<Box<dyn warp::Reply>, Infallible> {
        match self.db.link.get_all().await {
            Ok(links) => {
                match self.handlebars.render(
                    "all",
                    &serde_json::json!({"links": links, "go": self.host, "parent": PARENT_PARTIAL}),
                ) {
                    Ok(response) => Ok(Box::new(warp::reply::html(response))),
                    Err(e) => {
                        tracing::error!("{e}");
                        redirect("/.all")
                    }
                }
            }
            Err(e) => {
                tracing::error!("{e}");
                redirect("/.all")
            }
        }
    }

    pub async fn create(&self, request: CreateUpdateRequest, xsrf: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        if xsrf != self.xsrf() {
            return redirect("/");
        }

        let links: Vec<model::Link> = self.db.link.get_all().await.unwrap();
        for link in links.iter() {
            if request.short == link.short {
                return Ok(Box::new(warp::http::StatusCode::BAD_REQUEST));
            }
        }

        let link: model::Link = request.into();
        match self.db.link.insert(&link).await {
            Ok(id) => match self.db.link.get_by_id(&id).await {
                Ok(_) => redirect("/"),
                Err(e) => {
                    tracing::error!("{e}");
                    redirect("/")
                }
            },
            Err(e) => {
                tracing::error!("{e}");
                redirect("/")
            }
        }
    }

    pub async fn new_link(&self, request: CreateUpdateRequest) -> Result<Box<dyn warp::Reply>, Infallible> {
        let links: Vec<model::Link> = self.db.link.get_all().await.unwrap();
        for link in links.iter() {
            if request.short == link.short {
                return response(
                    "Link with short code already exists",
                    warp::http::StatusCode::BAD_REQUEST,
                );
            }
        }
        let link: model::Link = request.into();
        match self.db.link.insert(&link).await {
            Ok(id) => match self.db.link.get_by_id(&id).await {
                Ok(link) => Ok(Box::new(warp::reply::json(&link))),
                Err(e) => {
                    tracing::error!("{e}");
                    response(&e.to_string(), warp::http::StatusCode::INTERNAL_SERVER_ERROR)
                }
            },
            Err(e) => {
                tracing::error!("{e}");
                response(&e.to_string(), warp::http::StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    pub async fn update(
        &self,
        id: &Uuid,
        request: CreateUpdateRequest,
        xsrf: &str,
    ) -> Result<Box<dyn warp::Reply>, Infallible> {
        if xsrf != self.xsrf() {
            return redirect("/");
        }

        let links = self.db.link.get_all().await.unwrap();
        let id_list: Vec<Uuid> = links.iter().map(|l| l.id).collect();
        if !id_list.contains(id) {
            return redirect(&format!("/.detail/{}", id));
        }

        match self.db.link.get_by_id(id).await {
            Ok(link) => {
                let updated_link: model::Link = model::Link {
                    id: link.id,
                    short: request.short,
                    long: request.target,
                    created: link.created,
                    updated: chrono::Utc::now(),
                };
                match self.db.link.update(&updated_link).await {
                    Ok(()) => redirect(&format!("/.detail/{}", id)),
                    Err(e) => {
                        tracing::error!("{e}");
                        redirect(&format!("/.detail/{}", id))
                    }
                }
            }
            Err(e) => {
                tracing::error!("{e}");
                redirect(&format!("/.detail/{}", id))
            }
        }
    }

    pub async fn delete(&self, id: &Uuid, xsrf: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        if xsrf != self.xsrf() {
            return redirect("/");
        }

        let links = self.db.link.get_all().await.unwrap();
        let id_list: Vec<Uuid> = links.iter().map(|l| l.id).collect();
        if !id_list.contains(id) {
            return redirect("/");
        }

        match self.db.link.delete(id).await {
            Ok(()) => redirect("/"),
            Err(e) => {
                tracing::error!("{e}");
                redirect(&format!("/.detail/{}", id))
            }
        }
    }

    pub async fn help(&self) -> Result<Box<dyn warp::Reply>, Infallible> {
        match self
            .handlebars
            .render("help", &serde_json::json!({"go": self.host, "parent": PARENT_PARTIAL}))
        {
            Ok(response) => Ok(Box::new(warp::reply::html(response))),
            Err(e) => {
                tracing::error!("{e}");
                redirect("/")
            }
        }
    }

    pub async fn export(&self) -> Result<Box<dyn warp::Reply>, Infallible> {
        use serde_jsonlines::WriteExt;

        match self.db.link.get_all().await {
            Ok(links) => {
                let buffer = Vec::new();
                let mut writer = BufWriter::new(buffer);
                writer.write_json_lines(links).unwrap();
                writer.flush().expect("Unable to flush writer");
                let inner_buffer = writer.into_inner().unwrap();
                let result_string = String::from_utf8(inner_buffer).expect("Buffer content was not valid UTF-8");
                Ok(Box::new(warp::reply::with_status(
                    warp::reply::with_header(warp::reply::html(result_string), "Content-Type", "application/x-ndjson"),
                    warp::http::StatusCode::OK,
                )))
            }
            Err(e) => {
                tracing::error!("{e}");
                redirect("/")
            }
        }
    }

    pub async fn get(
        &self,
        short: &str,
        full_path: &str,
        query_params: HashMap<String, String>,
    ) -> Result<Box<dyn warp::Reply>, Infallible> {
        let link_id = model::normalized_id(short);
        match self.db.link.get(&link_id).await {
            Ok(link) => {
                let replacement_slug = if short.starts_with("/") {
                    short
                } else {
                    &format!("/{}", short)
                };
                let path = Renderer::path_remainder(full_path, replacement_slug);
                match self.expand_link(&path, query_params, &link.long) {
                    Ok(location) => {
                        match self.db.stats.incr(&link.id).await {
                            Ok(()) => {}
                            Err(e) => {
                                tracing::error!("{e}")
                            }
                        }
                        Ok(Box::new(warp::reply::with_status(
                            warp::redirect(location.parse::<warp::http::Uri>().unwrap()),
                            warp::http::StatusCode::PERMANENT_REDIRECT,
                        )))
                    }
                    Err(e) => {
                        tracing::error!("{e}");
                        let reply =
                            warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
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

    pub async fn json_detail(&self, short: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        match self.db.link.get(short).await {
            Ok(link) => match self.db.stats.get(&link.id).await {
                Ok(stats) => {
                    let details = model::LinkDetails {
                        id: link.id,
                        short: link.short,
                        long: link.long,
                        created: link.created,
                        updated: link.updated,
                        clicks: stats.map(|s| s.clicks.unwrap_or(0)),
                    };
                    Ok(Box::new(warp::reply::json(&details)))
                }
                Err(e) => {
                    tracing::error!("{e}");
                    Ok(Box::new(warp::reply::with_status(
                        warp::reply(),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )))
                }
            },
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::reply::with_status(
                    warp::reply(),
                    warp::http::StatusCode::NOT_FOUND,
                )))
            }
        }
    }

    pub async fn bad_request(&self) -> Result<Box<dyn warp::Reply>, Infallible> {
        Ok(Box::new(warp::http::StatusCode::BAD_REQUEST))
    }

    fn path_remainder(full_path: &str, short_slug: &str) -> String {
        match full_path.find(short_slug) {
            Some(start_index) => {
                let end_index = start_index + short_slug.len();
                let before_match = &full_path[..start_index];
                let after_match = &full_path[end_index..];
                format!("{}{}", before_match, after_match)
            }
            None => full_path.to_string(),
        }
    }

    pub(crate) fn expand_link(
        &self,
        path: &str,
        query_params: HashMap<String, String>,
        long: &str,
    ) -> Result<String, handlebars::RenderError> {
        let template = if !long.contains("{{") && !path.is_empty() {
            if long.ends_with("/") {
                format!("{}{}", long, "{{path}}")
            } else {
                format!("{}/{}", long, "{{path}}")
            }
        } else {
            long.to_string()
        };
        self.handlebars
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
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use url::Url;

    use super::*;

    #[test]
    fn test_encode() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link(
                "",
                HashMap::new(),
                "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}",
            )
            .unwrap();
        assert_eq!(res, "https://www.google.com/");
    }

    #[test]
    fn test_encode_1() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link(
                "Tolstoy",
                HashMap::new(),
                "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}",
            )
            .unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Tolstoy");
    }

    #[test]
    fn test_encode_2() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link(
                "Foo Bar baz",
                HashMap::new(),
                "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}",
            )
            .unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Foo%20Bar%20baz");
    }

    #[test]
    fn test_with_query_string() {
        let mut query_params = HashMap::new();
        query_params.insert("a".to_string(), "1".to_string());
        query_params.insert("bb".to_string(), "2".to_string());

        let renderer = Renderer::empty();
        let res = renderer
            .expand_link(
                "Foo Bar baz",
                query_params,
                "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}",
            )
            .unwrap();
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
        let renderer = Renderer::empty();
        let res = renderer
            .handlebars
            .render_template(
                "https://www.nytimes.com/{{#if path}}{{path}}{{/if}}",
                &serde_json::json!({"path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025/09/05/theater/broadway.html");
    }

    #[test]
    fn test_path_escape() {
        let renderer = Renderer::empty();
        let res = renderer
            .handlebars
            .render_template(
                "https://www.nytimes.com/{{#if path}}{{encode path}}{{/if}}",
                &serde_json::json!({"path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025%2F09%2F05%2Ftheater%2Fbroadway.html");
    }

    #[test]
    fn test_to_lower() {
        let renderer = Renderer::empty();
        let res = renderer
            .handlebars
            .render_template(
                "{{#if path}}{{lowercase path}}{{/if}}",
                &serde_json::json!({"path": "SAMIAM"}),
            )
            .unwrap();
        assert_eq!(res, "samiam");
    }

    #[test]
    fn test_to_upper() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("samiam", HashMap::new(), "{{#if path}}{{uppercase path}}{{/if}}")
            .unwrap();
        assert_eq!(res, "SAMIAM");
    }

    #[test]
    fn test_trim_suffix() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("a/", HashMap::new(), "{{#if path}}{{trimsuffix path '/'}}{{/if}}")
            .unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_trim_suffix_1() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("hello, world", HashMap::new(), "{{trimsuffix path ', world'}}")
            .unwrap();
        assert_eq!(res, "hello");
    }

    #[test]
    fn test_trim_suffix_2() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("", HashMap::new(), "{{#if path}}{{trimsuffix path '/'}}{{/if}}")
            .unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_prefix() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("OOOa", HashMap::new(), "{{#if path}}{{trimprefix path 'OOO'}}{{/if}}")
            .unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_prefix_1() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("hello, world", HashMap::new(), "{{trimprefix path 'hello, '}}")
            .unwrap();
        assert_eq!(res, "world");
    }

    #[test]
    fn test_prefix_2() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("", HashMap::new(), "{{#if path}}{{trimprefix path '/'}}{{/if}}")
            .unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_now_with_path() {
        let renderer = Renderer::empty();
        let res = renderer.expand_link("foobar", HashMap::new(), "{{ now }}").unwrap();
        assert!(!res.is_empty());
        // res should just be the date -- no path in template
        let parsed: chrono::DateTime<Utc> = res.parse().unwrap();
        assert!(parsed.type_id() == std::any::TypeId::of::<chrono::DateTime<Utc>>());
    }

    #[test]
    fn test_now_no_path() {
        let renderer = Renderer::empty();
        let res = renderer.expand_link("", HashMap::new(), "{{ now }}").unwrap();
        assert!(!res.is_empty());
        // res should just be the date -- no path in template
        let parsed: chrono::DateTime<Utc> = res.parse().unwrap();
        assert!(parsed.type_id() == std::any::TypeId::of::<chrono::DateTime<Utc>>());
    }

    #[test]
    fn test_match() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link(
                "123",
                HashMap::new(),
                r#"http://host.com/{{#if (match "\\d+" path)}}id/{{path}}{{else}}search/{{path}}{{/if}}"#,
            )
            .unwrap();
        assert_eq!(res, "http://host.com/id/123");
    }

    #[test]
    fn test_match_1() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link(
                "foo",
                HashMap::new(),
                r#"http://host.com/{{#if (match "\\d+" path)}}id/{{path}}{{else}}search/{{path}}{{/if}}"#,
            )
            .unwrap();
        assert_eq!(res, "http://host.com/search/foo");
    }

    #[test]
    fn test_no_mangle_escapes() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("", HashMap::new(), "http://host.com/foo%2f/bar")
            .unwrap();
        assert_eq!(res, "http://host.com/foo%2f/bar");
    }

    #[test]
    fn test_no_mangle_escapes_with_path() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("extra", HashMap::new(), "http://host.com/foo%2f/bar")
            .unwrap();
        assert_eq!(res, "http://host.com/foo%2f/bar/extra");
    }

    #[test]
    fn test_remainder() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("extra", HashMap::new(), "http://host.com/foo")
            .unwrap();
        assert_eq!(res, "http://host.com/foo/extra");
    }

    #[test]
    fn test_remainder_with_slash() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("extra", HashMap::new(), "http://host.com/foo/")
            .unwrap();
        assert_eq!(res, "http://host.com/foo/extra");
    }

    #[test]
    fn test_now_format() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link(
                "",
                HashMap::new(),
                r#"https://roamresearch.com/#/app/ts-corp/page/{{ nowformat "%d/%m/%Y"}}"#,
            )
            .unwrap();
        let path = res.strip_prefix("https://").unwrap();
        let segments: Vec<&str> = path.split("/").collect();
        assert!(segments.len() == 8); // roamresearch.com + # + app + ts-corp + page + d + m + Y = 8
    }

    #[test]
    fn test_undefined_field() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("bar", HashMap::new(), "http://host.com/{{ bar }}")
            .unwrap();
        assert_eq!(res, "http://host.com/");
    }

    #[test]
    fn test_defined_field() {
        let renderer = Renderer::empty();
        let res = renderer
            .expand_link("bar", HashMap::new(), "http://host.com/{{path}}")
            .unwrap();
        assert_eq!(res, "http://host.com/bar");
    }

    #[test]
    fn test_trim_short_1() {
        let original_path = "nyt/sports/article";
        let slug_to_remove = "nyt";
        let result = Renderer::path_remainder(original_path, slug_to_remove);
        assert_eq!(result, "/sports/article");
    }

    #[test]
    fn test_trim_short_2() {
        let original_id = "post-123-post-456";
        let first_post = "post-";
        let result = Renderer::path_remainder(original_id, first_post);
        assert_eq!(result, "123-post-456");
    }

    #[test]
    fn test_trim_short_3() {
        let original_text = "Hello World";
        let not_found = "Rust";
        let result = Renderer::path_remainder(original_text, not_found);
        assert_eq!(result, original_text);
    }

    #[test]
    fn test_trim_short_4() {
        let original_path = "/nyt";
        let slug_to_remove = "/nyt";
        let result = Renderer::path_remainder(original_path, slug_to_remove);
        assert_eq!(result, "");
    }
}
