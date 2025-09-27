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
    // construct a subscriber that prints formatted traces to stdout
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    // use that subscriber to process traces emitted after this point
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
        .register_template_file("detail", format!("{}/detail.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("help", format!("{}/help.hbs", args.templates_dir))
        .unwrap();
    handlebars
        .register_template_file("home", format!("{}/home.hbs", args.templates_dir))
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
