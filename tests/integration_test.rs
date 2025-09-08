use std::collections::HashMap;

use gohome::{CreateUpdateRequest, db, handlers, model};
use rusqlite::Connection;
use warp::reply::Reply;

#[tokio::test]
async fn test_db() -> Result<(), Box<dyn std::error::Error + 'static>> {
    let connection = Connection::open_in_memory()?;
    let db = db::Db::new(connection)?;

    ////// Links
    let link_created = chrono::Utc::now();
    let test_link = model::Link {
        id: "nyt".to_string(),
        short: "nyt".to_string(),
        long: "https://nytimes.com".to_string(),
        created: link_created.clone(),
        updated: chrono::Utc::now(),
        owner: Some("christopher".to_string()),
    };

    // Insert
    let result = db.link.insert(&test_link).await?;
    assert_eq!(result, test_link.id);
    // A click stats should be created with null clicks
    let clicks = db.stats.get(&test_link.id).await?;
    assert!(clicks.is_some());
    assert!(clicks.unwrap().clicks.is_none());

    // Get
    let from_db_link = db.link.get(&result).await?;
    assert_eq!(test_link, from_db_link);
    assert_eq!(from_db_link.short, test_link.short);

    // Get All
    let all_links = db.link.get_all().await?;
    assert_eq!(all_links.len(), 1);
    assert_eq!(*all_links.get(&result).unwrap(), test_link);

    // Update
    let updated_link = model::Link {
        id: "nyt".to_string(),
        short: "nytimes".to_string(),
        long: "https://nytimes.com".to_string(),
        created: link_created.clone(),
        updated: chrono::Utc::now(),
        owner: Some("christopher".to_string()),
    };
    let _ = db.link.update(&updated_link).await?;
    let read_updated = db.link.get(&result).await?;
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

    let create_request = CreateUpdateRequest {
        short: "nyt".to_string(),
        target: "https://nytimes.com".to_string(),
        owner: Some("christopher".to_string()),
    };
    let mut reply = handlers::create(create_request, db.clone()).await?;
    let mut response = reply.into_response();
    assert_eq!(response.status(), warp::http::StatusCode::CREATED);
    assert_eq!(response.headers().get("Location").unwrap().to_str().unwrap(), "/nyt");

    let mut from_db_link = db.link.get("nyt").await?;
    // println!("{}", &from_db_link);
    assert_eq!(from_db_link.short, "nyt");

    reply = handlers::get("nyt", "/nyt", HashMap::new(), db.clone()).await?;
    response = reply.into_response();
    assert_eq!(response.status(), warp::http::StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        response.headers().get("Location").unwrap().to_str().unwrap(),
        "https://nytimes.com/"
    );

    let update_request = CreateUpdateRequest {
        short: "nyt".to_string(),
        target: "https://example.com".to_string(),
        owner: Some("christopher".to_string()),
    };
    reply = handlers::update(&model::normalized_id(&update_request.short), update_request, db.clone()).await?;
    response = reply.into_response();
    assert_eq!(response.status(), warp::http::StatusCode::OK);
    assert_eq!(response.headers().get("Location").unwrap().to_str().unwrap(), "/nyt");

    from_db_link = db.link.get("nyt").await?;
    assert_eq!(from_db_link.short, "nyt");
    assert_eq!(from_db_link.long, "https://example.com");

    let all_links = db.link.get_all().await?;
    assert_eq!(all_links.len(), 1);

    reply = handlers::all(db.clone()).await?;
    response = reply.into_response();
    let all_body = response.into_body();
    let all_body_bytes = all_body.collect().await?;
    let all_body_string = String::from_utf8(all_body_bytes.to_bytes().to_vec())?;
    let links: HashMap<String, model::Link> = serde_json::from_str(&all_body_string)?;
    assert!(links.len() == 1);
    assert!(links.get("nyt").is_some_and(|link| link.long == "https://example.com"));

    reply = handlers::detail("nyt", db.clone()).await?;
    response = reply.into_response();
    let mut details_body = response.into_body();
    let mut details_body_bytes = details_body.collect().await?;
    let mut details_body_string = String::from_utf8(details_body_bytes.to_bytes().to_vec())?;
    let mut details_response: gohome::DetailsResponse = serde_json::from_str(&details_body_string)?;
    assert!(details_response.stats.is_some_and(|s| s.clicks.is_none()));

    db.stats.incr("nyt").await?;
    reply = handlers::detail("nyt", db.clone()).await?;
    response = reply.into_response();
    details_body = response.into_body();
    details_body_bytes = details_body.collect().await?;
    details_body_string = String::from_utf8(details_body_bytes.to_bytes().to_vec())?;
    details_response = serde_json::from_str(&details_body_string)?;
    assert!(details_response.stats.is_some_and(|s| s.clicks.is_some_and(|c| c == 1)));

    Ok(())
}
