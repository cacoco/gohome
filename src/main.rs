use std::{net::SocketAddr, path::Path, time::Duration};

use clap::Parser;
use shadow_rs::shadow;
use tracing_subscriber::EnvFilter;

shadow!(build);

#[derive(Parser, Debug)]
#[command(version = build::VERSION, long_version = build::CLAP_LONG_VERSION, about = "", long_about = "")]
struct Args {
    #[arg(long, help = "", default_value = "127.0.0.1:3030")]
    host: SocketAddr,
    #[arg(long, help = "", default_value = "/home/nonroot")]
    sqlite_path: String,
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
    let base_path = Path::new(&args.sqlite_path);
    let database_path_binding = base_path.join("gohome.db");
    let db_path = database_path_binding.as_path();
    let connection = rusqlite::Connection::open(db_path.to_str().unwrap())?; // we want this to fail loudly

    let db = gohome::db::Db::new(connection).unwrap();
    let routes = gohome::routes::get_routes(db);

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
