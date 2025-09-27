use std::collections::HashMap;

use gohome::{db, model, render::Renderer, CreateUpdateRequest};
use rusqlite::Connection;
use uuid::Uuid;
use warp::reply::Reply;

#[tokio::test]
async fn test_db() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let connection = Connection::open_in_memory()?;
    let db = db::Db::new(connection)?;

    ////// Links
    let link_created = chrono::Utc::now();
    let test_link = model::Link {
        id: Uuid::new_v4(),
        short: "nyt".to_string(),
        long: "https://nytimes.com".to_string(),
        created: link_created.clone(),
        updated: chrono::Utc::now(),
    };

    // Insert
    let result = db.link.insert(&test_link).await?;
    assert_eq!(result, test_link.id);
    // A click stats should be created with null clicks
    let clicks = db.stats.get(&test_link.id).await?;
    assert!(clicks.is_some());
    assert!(clicks.unwrap().clicks.is_none());

    // Get
    let from_db_link = db.link.get_by_id(&result).await?;
    assert_eq!(test_link, from_db_link);
    assert_eq!(from_db_link.short, test_link.short);

    // Get All
    let all_links = db.link.get_all().await?;
    assert_eq!(all_links.len(), 1);
    assert_eq!(*all_links.first().unwrap(), test_link);

    // Update
    let updated_link = model::Link {
        id: result.clone(),
        short: "nytimes".to_string(),
        long: "https://nytimes.com".to_string(),
        created: link_created.clone(),
        updated: chrono::Utc::now(),
    };
    let _ = db.link.update(&updated_link).await?;
    let read_updated = db.link.get_by_id(&result).await?;
    assert_eq!(read_updated.short, updated_link.short);

    ////// Stats INCR
    let mut stats = db.stats.get(&test_link.id).await?;
    assert!(stats.is_some());
    assert!(stats.unwrap().clicks.is_none());
    db.stats.incr(&test_link.id).await?;
    stats = db.stats.get(&test_link.id).await?;
    assert!(stats.is_some());
    assert!(stats.unwrap().clicks.is_some_and(|clicks| clicks == 1));
    db.stats.incr(&test_link.id).await?;
    stats = db.stats.get(&test_link.id).await?;
    assert!(stats.is_some());
    assert!(stats.unwrap().clicks.is_some_and(|clicks| clicks == 2));
    db.stats.incr(&test_link.id).await?;
    stats = db.stats.get(&test_link.id).await?;
    assert!(stats.is_some());
    assert!(stats.unwrap().clicks.is_some_and(|clicks| clicks == 3));

    Ok(())
}

#[tokio::test]
async fn test_handlers() -> Result<(), Box<dyn std::error::Error + 'static>> {
    use http_body_util::BodyExt;

    let connection = Connection::open_in_memory()?;
    let db = db::Db::new(connection)?;

    let mut handlebars = handlebars::Handlebars::new();
    // configure handlerbars
    handlebars.register_template_file("base", "./templates/base.hbs").unwrap();
    handlebars.register_template_file("home", "./templates/home.hbs").unwrap();
    handlebars.register_template_file("all", "./templates/all.hbs").unwrap();
    let renderer = Renderer::new(db.clone(), handlebars);

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
    reply = renderer.update(&Uuid::new_v4(), update_request, &renderer.xsrf()).await?;
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
