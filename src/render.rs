use std::{collections::HashMap, convert::Infallible, io::{BufWriter, Write}};

use csrf::{AesGcmCsrfProtection, CsrfProtection};
use rand::RngCore;
use chrono::{DateTime, Utc};
use handlebars::Handlebars;
use regex::Regex;
use uuid::Uuid;

use crate::{db, model::{self, Popular}, CreateUpdateRequest};

handlebars::handlebars_helper!(encode: |query: String| urlencoding::encode(&query).to_owned());
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
handlebars::handlebars_helper!(now: |*kwargs| {
    tracing::trace!("{:?}", kwargs);
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
    // templates_dir: String,
    csrf_token: csrf::CsrfToken,
    pub(crate) db: db::Db,
    pub(crate) handlebars: handlebars::Handlebars<'static>
}

impl Renderer {
    pub fn default() -> Self {
        Self::new(db::Db::default().unwrap(), Handlebars::new())
    }

    pub fn new(db: db::Db, handlebars: handlebars::Handlebars<'static>) -> Self {
        let mut secret_key = [0u8; 32];
        rand::rng().fill_bytes(&mut secret_key);
        let protect = AesGcmCsrfProtection::from_key(secret_key);

        let mut nonce = [0u8; 64];
        rand::rng().fill_bytes(&mut nonce);
        let csrf_token: csrf::CsrfToken = protect.generate_token(&mut nonce).unwrap();

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
            // templates_dir: templates_dir.to_string(),
            csrf_token,
            db: db,
            handlebars: bars
        }
    }

    pub fn xsrf(&self) -> String {
        self.csrf_token.b64_string()
    }
}

impl Renderer {
    pub async fn home(&self) -> Result<Box<dyn warp::Reply>, Infallible> {
        let mut links: Vec<(model::Link, model::ClickStats)> = Vec::new();
        match self.db.link.most_popular().await {
            Ok(mut results) => {
                links.append(&mut results);
            },
            Err(e) => {
                tracing::error!("{e}");
            }
        }

        let most_popular_links: Vec<model::Popular> = links.iter().map(|(link, stats)| {
            Popular { 
                id: link.id.to_string(),
                short: link.short.clone(),
                clicks: stats.clicks.or(Some(0))
            }
        }).collect();
        match self.handlebars.render("home", &serde_json::json!({"go": "go", "parent": "base", "links": most_popular_links, "XSRF": self.xsrf()})) {
            Ok(response) => {
                Ok(Box::new(warp::reply::html(response)))
            },
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())))  
            }
        }
    }

    pub async fn detail(&self, short: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        match self.db.link.get(&short).await {
            Ok(link) => {
                match self.handlebars.render("detail", &serde_json::json!({"go": "go", "parent": "base", "link": link, "XSRF": self.xsrf()})) {
                    Ok(response) => {
                        Ok(Box::new(warp::reply::html(response)))
                    },
                    Err(e) => {
                        tracing::error!("{e}");
                        Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())))  
                    }
                }
            }
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())))
            }
        }
    }

    pub async fn all(&self) -> Result<Box<dyn warp::Reply>, Infallible> {
        match self.db.link.get_all().await {
            Ok(links) => {
                match self.handlebars.render("all", &serde_json::json!({"links": links, "go": "go", "parent": "base"})) {
                    Ok(response) => {
                        Ok(Box::new(warp::reply::html(response)))
                    },
                    Err(e) => {
                        tracing::error!("{e}");
                        Ok(Box::new(warp::redirect("/.all".parse::<warp::http::Uri>().unwrap())))  
                    }
                }
            },
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::redirect("/.all".parse::<warp::http::Uri>().unwrap())))
            }
        }
    }

    pub async fn create(&self, request: CreateUpdateRequest, xsrf: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        if xsrf != self.xsrf() {
            return Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())));
        }

        let links: Vec<model::Link> = self.db.link.get_all().await.unwrap();
        for link in links.iter() {
            if request.short == link.short {
                return Ok(Box::new(warp::http::StatusCode::BAD_REQUEST));
            }
        }

        let link: model::Link = request.into();
        tracing::debug!("creating new link: {:#?}", &link);
        match self.db.link.insert(&link).await {
            Ok(id) => {
                tracing::trace!("saved new db entry with id: {}", id);
                match self.db.link.get_by_id(&id).await {
                    Ok(created) => {
                        tracing::info!("successfully created new link with id: {}", &created.id);
                        Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())))
                    },
                    Err(e) => {
                        tracing::error!("{e}");
                        Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())))
                    }
                }
            },
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())))
            }
        }
    }

    pub async fn update(&self, id: &Uuid, request: CreateUpdateRequest, xsrf: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        if xsrf != self.xsrf() {
            return Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())));
        }

        let links = self.db.link.get_all().await.unwrap();
        let id_list: Vec<Uuid> = links.iter().map(|l| l.id.clone()).collect();
        if !id_list.contains(&id) {
            return Ok(Box::new(warp::redirect(format!("/.detail/{}",id).parse::<warp::http::Uri>().unwrap())));
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
                tracing::debug!("updating link id: {id}");
                match self.db.link.update(&updated_link).await {
                    Ok(()) => {
                        tracing::info!("updated db entry with id: {}", id);
                        Ok(Box::new(warp::redirect(format!("/.detail/{}",id).parse::<warp::http::Uri>().unwrap())))
                    },
                    Err(e) => {
                        tracing::error!("{e}");
                        Ok(Box::new(warp::redirect(format!("/.detail/{}",id).parse::<warp::http::Uri>().unwrap())))
                    }
                }
            },
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::redirect(format!("/.detail/{}",id).parse::<warp::http::Uri>().unwrap())))
            }
        }
    }

    pub async fn delete(&self, id: &Uuid, xsrf: &str) -> Result<Box<dyn warp::Reply>, Infallible> {
        if xsrf != self.xsrf() {
            return Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())));
        }

        let links = self.db.link.get_all().await.unwrap();
        let id_list: Vec<Uuid> = links.iter().map(|l| l.id.clone()).collect();
        if !id_list.contains(&id) {
            return Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())));
        }

        match self.db.link.delete(id).await {
            Ok(()) => Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap()))),
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::redirect(format!("/.detail/{}",id).parse::<warp::http::Uri>().unwrap())))
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
                Ok(Box::new(warp::reply::with_status(warp::reply::with_header(warp::reply::html(result_string), "Content-Type", "application/x-ndjson"), warp::http::StatusCode::OK)))
            },
            Err(e) => {
                tracing::error!("{e}");
                Ok(Box::new(warp::redirect("/".parse::<warp::http::Uri>().unwrap())))
            }
        }
    }

    pub async fn get(
        &self,
        short: &str,
        full_path: &str,
        query_params: HashMap<String, String>,
    ) -> Result<Box<dyn warp::Reply>, Infallible> {
        tracing::info!("full path: {}", full_path);
        tracing::info!("query params: {:#?}", query_params);

        let link_id = model::normalized_id(short);
        match self.db.link.get(&link_id).await {
            Ok(link) => {
                let replacement_slug = if short.starts_with("/") {
                    short
                } else {
                    &format!("/{}", short)
                };
                tracing::info!("replacement slug: {replacement_slug}");
                let path = Renderer::trim_short(full_path, replacement_slug);
                tracing::info!("resulting path: {path}");
                match self.expand_link(&path, query_params, &link.long) {
                    Ok(location) => {
                        match self.db.stats.incr(&link.id).await {
                            Ok(()) => tracing::debug!("Successfully updated click stats for link: {}", &link.id),
                            Err(e) => tracing::error!("{e}")
                        }
                        Ok(Box::new(warp::reply::with_status(warp::redirect(location.parse::<warp::http::Uri>().unwrap()), warp::http::StatusCode::PERMANENT_REDIRECT)))
                    },
                    Err(e) => {
                        tracing::error!("{}", e);
                        let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                        Ok(Box::new(reply))
                    }
                }
            },
            Err(e) => {
                tracing::error!("{e}");
                let reply = warp::reply::with_status(warp::reply(), warp::http::StatusCode::NOT_FOUND);
                Ok(Box::new(reply))
            }
        }
    }

    /// Replaces the first occurrence of `text_to_replace` within `full_string`
    /// with an empty string.
    ///
    /// If `text_to_replace` is not found, the original string is returned.
    ///
    /// # Arguments
    /// * `full_string` - The original string slice to search within.
    /// * `text_to_replace` - The string slice whose first occurrence should be removed.
    ///
    /// # Returns
    /// A new `String` with the replacement applied, or a copy of the original string.
    pub(crate) fn trim_short(full_string: &str, text_to_replace: &str) -> String {
        // 1. Use the `find` method to get the byte index of the start of the first match.
        match full_string.find(text_to_replace) {
            Some(start_index) => {
                // 2. Calculate the byte index where the replacement text ends.
                let end_index = start_index + text_to_replace.len();

                // 3. Take the slice of the string *before* the match.
                let before_match = &full_string[..start_index];
                
                // 4. Take the slice of the string *after* the match.
                let after_match = &full_string[end_index..];

                // 5. Concatenate the two parts and return the new String.
                let r = format!("{}{}", before_match, after_match);
                if r.starts_with("/") {
                    r.chars().next().map_or(r.clone(), |c| r[c.len_utf8()..].to_string())
                } else {
                    r
                }
            }
            None => {
                // If the text is not found, return a copy of the original string.
                full_string.to_string()
            }
        }
    }

    pub(crate) fn expand_link(
        &self,
        path: &str,
        query_params: HashMap<String, String>,
        long: &str,
    ) -> Result<String, handlebars::RenderError> {
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
        tracing::info!("expand template: {template}");
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
        let renderer = Renderer::default();
        let res = renderer.expand_link("", HashMap::new(), "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
        assert_eq!(res, "https://www.google.com/");
    }

    #[test]
    fn test_encode_1() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("Tolstoy", HashMap::new(), "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Tolstoy");
    }

    #[test]
    fn test_encode_2() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("Foo Bar baz", HashMap::new(), "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
        assert_eq!(res, "https://www.google.com/search?q=Foo%20Bar%20baz");
    }

    #[test]
    fn test_with_query_string() {
        let mut query_params = HashMap::new();
        query_params.insert("a".to_string(), "1".to_string());
        query_params.insert("bb".to_string(), "2".to_string());
        
        let renderer = Renderer::default();
        let res = renderer.expand_link("Foo Bar baz", query_params, "https://www.google.com/{{#if path}}search?q={{encode path}}{{/if}}").unwrap();
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
        let renderer = Renderer::default();
        let res = renderer.handlebars
            .render_template(
                "https://www.nytimes.com/{{#if path}}{{path}}{{/if}}",
                &serde_json::json!({"path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025/09/05/theater/broadway.html");
    }

    #[test]
    fn test_path_escape() {
        let renderer = Renderer::default();
        let res = renderer.handlebars
            .render_template(
                "https://www.nytimes.com/{{#if path}}{{encode path}}{{/if}}",
                &serde_json::json!({"path": "2025/09/05/theater/broadway.html"}),
            )
            .unwrap();
        assert_eq!(res, "https://www.nytimes.com/2025%2F09%2F05%2Ftheater%2Fbroadway.html");
    }

    #[test]
    fn test_to_lower() {
        let renderer = Renderer::default();
        let res = renderer.handlebars
            .render_template(
                "{{#if path}}{{lowercase path}}{{/if}}",
                &serde_json::json!({"path": "SAMIAM"}),
            )
            .unwrap();
        assert_eq!(res, "samiam");
    }

    #[test]
    fn test_to_upper() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("samiam", HashMap::new(), "{{#if path}}{{uppercase path}}{{/if}}").unwrap();
        assert_eq!(res, "SAMIAM");
    }

    #[test]
    fn test_trim_suffix() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("a/", HashMap::new(), "{{#if path}}{{trimsuffix path '/'}}{{/if}}").unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_trim_suffix_1() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("hello, world", HashMap::new(), "{{trimsuffix path ', world'}}").unwrap();
        assert_eq!(res, "hello");
    }

    #[test]
    fn test_trim_suffix_2() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("", HashMap::new(), "{{#if path}}{{trimsuffix path '/'}}{{/if}}").unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_prefix() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("OOOa", HashMap::new(),"{{#if path}}{{trimprefix path 'OOO'}}{{/if}}").unwrap();
        assert_eq!(res, "a");
    }

    #[test]
    fn test_prefix_1() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("hello, world", HashMap::new(), "{{trimprefix path 'hello, '}}").unwrap();
        assert_eq!(res, "world");
    }

    #[test]
    fn test_prefix_2() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("", HashMap::new(), "{{#if path}}{{trimprefix path '/'}}{{/if}}").unwrap();
        assert_eq!(res, "");
    }

    #[test]
    fn test_now_with_path() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("foobar", HashMap::new(), "{{ now }}").unwrap();
        assert!(!res.is_empty());
        // res should just be the date -- no path in template
        let parsed: chrono::DateTime<Utc> = res.parse().unwrap();
        assert!(parsed.type_id() == std::any::TypeId::of::<chrono::DateTime<Utc>>());
    }

    #[test]
    fn test_now_no_path() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("", HashMap::new(), "{{ now }}").unwrap();
        assert!(!res.is_empty());
        // res should just be the date -- no path in template
        let parsed: chrono::DateTime<Utc> = res.parse().unwrap();
        assert!(parsed.type_id() == std::any::TypeId::of::<chrono::DateTime<Utc>>());
    }

    #[test]
    fn test_match() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("123", HashMap::new(),r#"http://host.com/{{#if (match "\\d+" path)}}id/{{path}}{{else}}search/{{path}}{{/if}}"#).unwrap();
        assert_eq!(res, "http://host.com/id/123");
    }

    #[test]
    fn test_match_1() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("foo", HashMap::new(), r#"http://host.com/{{#if (match "\\d+" path)}}id/{{path}}{{else}}search/{{path}}{{/if}}"#).unwrap();
        assert_eq!(res, "http://host.com/search/foo");
    }

    #[test]
    fn test_no_mangle_escapes() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("", HashMap::new(), "http://host.com/foo%2f/bar").unwrap();
        assert_eq!(res, "http://host.com/foo%2f/bar");
    }

    #[test]
    fn test_no_mangle_escapes_with_path() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("extra", HashMap::new(), "http://host.com/foo%2f/bar").unwrap();
        assert_eq!(res, "http://host.com/foo%2f/bar/extra");
    }

    #[test]
    fn test_remainder() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("extra", HashMap::new(), "http://host.com/foo").unwrap();
        assert_eq!(res, "http://host.com/foo/extra");
    }

    #[test]
    fn test_remainder_with_slash() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("extra", HashMap::new(), "http://host.com/foo/").unwrap();
        assert_eq!(res, "http://host.com/foo/extra");
    }

    #[test]
    fn test_now_format() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("",HashMap::new(), r#"https://roamresearch.com/#/app/ts-corp/page/{{ nowformat "%d/%m/%Y"}}"#).unwrap();
        let path = res.strip_prefix("https://").unwrap();
        let segments: Vec<&str> = path.split("/").collect();
        assert!(segments.len() == 8); // roamresearch.com + # + app + ts-corp + page + d + m + Y = 8
    }

    #[test]
    fn test_undefined_field() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("bar", HashMap::new(), "http://host.com/{{ bar }}").unwrap();
        assert_eq!(res, "http://host.com/");
    }

    #[test]
    fn test_defined_field() {
        let renderer = Renderer::default();
        let res = renderer.expand_link("bar", HashMap::new(), "http://host.com/{{path}}").unwrap();
        assert_eq!(res, "http://host.com/bar");
    }

    #[test]
    fn test_trim_short_1() {
        let original_path = "nyt/sports/article";
        let slug_to_remove = "nyt";
        let result = Renderer::trim_short(original_path, slug_to_remove);
        assert_eq!(result, "/sports/article");
    }

    #[test]
    fn test_trim_short_2() {
        let original_id = "post-123-post-456";
        let first_post = "post-";
        let result = Renderer::trim_short(original_id, first_post);
        assert_eq!(result, "123-post-456");
    }

    #[test]
    fn test_trim_short_3() {
        let original_text = "Hello World";
        let not_found = "Rust";
        let result = Renderer::trim_short(original_text, not_found);
        assert_eq!(result, original_text);
    }

    #[test]
    fn test_trim_short_4() {
        let original_path = "/nyt";
        let slug_to_remove = "/nyt";
        let result = Renderer::trim_short(original_path, slug_to_remove);
        assert_eq!(result, "");
    }
}
