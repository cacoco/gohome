use std::collections::HashMap;

use gohome::{CreateUpdateRequest, db::Db, render::Renderer};
use rusqlite::Connection;
use uuid::Uuid;
use warp::reply::Reply;

#[tokio::test]
async fn test_handlers() -> Result<(), Box<dyn std::error::Error + 'static>> {
    use http_body_util::BodyExt;

    let connection = Connection::open_in_memory()?;
    let db = Db::new(connection)?;

    let mut handlebars = handlebars::Handlebars::new();
    // configure handlerbars
    handlebars
        .register_template_file("base", "./templates/base.hbs")
        .unwrap();
    handlebars
        .register_template_file("home", "./templates/home.hbs")
        .unwrap();
    handlebars.register_template_file("all", "./templates/all.hbs").unwrap();
    let renderer = Renderer::new("go", db.clone(), handlebars);

    let create_request = CreateUpdateRequest {
        short: "nyt".to_string(),
        target: "https://nytimes.com".to_string(),
    };
    let mut reply = renderer.create(create_request, &renderer.xsrf()).await?;
    let mut response = reply.into_response();
    assert_eq!(response.status(), warp::http::StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(response.headers().get("Location").unwrap().to_str().unwrap(), "/nyt");

    let mut from_db_link = db.link.get("nyt").await?;
    assert_eq!(from_db_link.short, "nyt");

    reply = renderer.get("nyt", "/", HashMap::new()).await?;
    response = reply.into_response();
    assert_eq!(response.status(), warp::http::StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("Location").unwrap().to_str().unwrap(),
        "https://nytimes.com/"
    );

    let update_request = CreateUpdateRequest {
        short: "nyt".to_string(),
        target: "https://example.com".to_string(),
    };
    reply = renderer
        .update(&Uuid::new_v4(), update_request, &renderer.xsrf())
        .await?;
    response = reply.into_response();
    assert_eq!(response.status(), warp::http::StatusCode::OK);
    assert_eq!(response.headers().get("Location").unwrap().to_str().unwrap(), "/nyt");

    from_db_link = db.link.get("nyt").await?;
    assert_eq!(from_db_link.short, "nyt");
    assert_eq!(from_db_link.long, "https://example.com");

    let all_links = db.link.get_all().await?;
    assert_eq!(all_links.len(), 1);

    reply = renderer.all().await?;
    response = reply.into_response();
    let all_body = response.into_body();
    let all_body_bytes = all_body.collect().await?;
    let all_body_string = String::from_utf8(all_body_bytes.to_bytes().to_vec())?;
    println!("{all_body_string}");
    assert!(!all_body_string.is_empty());

    reply = renderer.detail("nyt").await?;
    response = reply.into_response();
    let details_body = response.into_body();
    let details_body_bytes = details_body.collect().await?;
    let details_body_string = String::from_utf8(details_body_bytes.to_bytes().to_vec())?;
    assert!(!details_body_string.is_empty());

    Ok(())
}
