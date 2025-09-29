use std::{net::SocketAddr, path::Path, time::Duration};

use clap::Parser;
use gohome::render::Renderer;
use handlebars::Handlebars;
use shadow_rs::shadow;
use tracing_subscriber::EnvFilter;

shadow!(build);

#[derive(Parser, Debug)]
#[command(version = build::VERSION, long_version = build::CLAP_LONG_VERSION, about = "", long_about = "")]
struct Args {
    #[arg(long, env = "DOMAIN", default_value = "go")]
    domain: String,
    #[arg(long, env = "HOST", default_value = "127.0.0.1:3030")]
    host: SocketAddr,
    #[arg(long, default_value = "/home/nonroot")]
    sqlite_path: String,
    #[arg(long, default_value = "/usr/src/templates")]
    templates_dir: String,
    #[arg(long, default_value = "/usr/src/assets")]
    assets_dir: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // construct a subscriber that prints to stdout
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    // use subscriber to process emitted events after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();
    tracing::info!("{:?}", &args);

    // database config
    let database_base_path = Path::new(&args.sqlite_path);
    let database_path_binding = database_base_path.join("gohome.db");
    let db_path = database_path_binding.as_path();
    let connection = rusqlite::Connection::open(db_path.to_str().unwrap())?; // we want this to fail loudly
    let db = gohome::db::Db::new(connection).unwrap();

    // templating config
    let mut handlebars = Handlebars::new();
    handlebars
        .register_template_file("all", format!("{}/all.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("base", format!("{}/base.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("delete", format!("{}/delete.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("detail", format!("{}/detail.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("help", format!("{}/help.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("home", format!("{}/home.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("success", format!("{}/success.hbs", args.templates_dir))
        .unwrap();

    let renderer = Renderer::new(&args.domain, db, handlebars);
    let routes = gohome::routes::get_routes(renderer, args.assets_dir);

    tracing::info!("starting warp server: {}", &args.host);
    tracing::info!("sqlitedb: {}", db_path.to_str().unwrap());
    warp::serve(routes)
        .bind(args.host)
        .await
        .graceful(async {
            tokio::signal::ctrl_c()
                .await
                .expect("\nfailed to install CTRL+C signal handler");
        })
        .run()
        .await;

    tracing::info!("gracefully exited.");
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, net::IpAddr};

    use gohome::model;

    use super::*;

    #[tokio::test]
    async fn test_api_routes() -> Result<(), Box<dyn std::error::Error>> {
        let renderer = Renderer::empty();
        let routes = gohome::routes::get_routes(renderer, "static".to_string());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let ipv4_addr = IpAddr::from([127, 0, 0, 1]);

        let handler = tokio::task::spawn(async move {
            warp::serve(routes).incoming(listener).run().await;
        });

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connection_verbose(true)
            .timeout(Duration::from_secs(2))
            .local_address(ipv4_addr)
            .build()?;

        // post to create
        let mut form_data: HashMap<String, String> = HashMap::new();
        form_data.insert("short".to_string(), "nyt".to_string());
        form_data.insert("long".to_string(), "http://www.nytimes.com".to_string());

        let post_request = client
            .post(format!("http://{}/", addr))
            .header("Sec-Golink", "1")
            .form(&form_data)
            .build()?;

        let post_response = client.execute(post_request).await?;
        assert_eq!(post_response.status(), warp::http::StatusCode::CREATED);
        let created_link = post_response.json::<model::Link>().await?;
        assert_eq!(created_link.short, "nyt".to_string());
        assert_eq!(created_link.long, "http://www.nytimes.com".to_string());

        // read details go/short+
        let read_request = client.get(format!("http://{}/nyt+", addr)).build()?;

        let read_response = client.execute(read_request).await?;
        assert_eq!(read_response.status(), warp::http::StatusCode::OK);
        let details = read_response.json::<model::LinkDetails>().await?;
        assert_eq!(details.short, "nyt".to_string());
        assert_eq!(details.long, "http://www.nytimes.com".to_string());
        assert_eq!(details.created, created_link.created);
        assert_eq!(details.updated, created_link.updated); // updated should be the same as created
        assert!(details.clicks.is_some_and(|s| s == 0));

        // trigger a click go/short
        let gohome_request = client.get(format!("http://{}/nyt", addr)).build()?;

        let gohome_response = client.execute(gohome_request).await?;
        assert_eq!(gohome_response.status(), warp::http::StatusCode::PERMANENT_REDIRECT);
        assert_eq!(
            gohome_response.headers().get("Location").unwrap().to_str().unwrap(),
            "http://www.nytimes.com/"
        );

        // read details go/short+ (again)
        let read_request_post_click = client.get(format!("http://{}/nyt+", addr)).build()?;

        let read_response_post_click = client.execute(read_request_post_click).await?;
        assert_eq!(read_response_post_click.status(), warp::http::StatusCode::OK);
        let details_post_click = read_response_post_click.json::<model::LinkDetails>().await?;
        assert!(details_post_click.clicks.is_some_and(|s| s == 1));

        // export go/.export
        let export_request = client.get(format!("http://{}/.export", addr)).build()?;

        let export_response = client.execute(export_request).await?;
        assert_eq!(export_response.status(), warp::http::StatusCode::OK);
        let export_bytes = export_response.bytes().await?;
        assert!(!export_bytes.is_empty());
        let exported_link = serde_json::from_slice::<model::Link>(&export_bytes)?;
        assert_eq!(exported_link.short, created_link.short);
        assert_eq!(exported_link.long, created_link.long);
        assert_eq!(exported_link.created, created_link.created);
        assert_eq!(exported_link.updated, details_post_click.updated); // updated should be the same as post-click

        handler.abort();
        Ok(())
    }
}
