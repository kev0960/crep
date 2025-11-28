use std::net::SocketAddr;
use std::path::Path;
use std::time::Instant;

use axum::serve;
use clap::Parser;
use crep_server::config::ServerConfig;
use crep_server::router;
use crep_server::server_context::ServerContext;
use crep_server::watch::ignore_checker::IgnoreChecker;
use crep_server::watch::repo_watcher::WatcherConfig;
use crep_server::watch::repo_watcher::init_watcher_and_indexer;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser, Debug)]
#[command(version, about = "Crep Server")]
struct Args {
    #[arg(short, long)]
    config: Option<String>,

    #[arg(long)]
    debug_level: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new(
                        args.debug_level.as_deref().unwrap_or("info"),
                    )
                }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config =
        ServerConfig::new(args.config.as_deref().unwrap_or("./config.yaml"))?;

    info!("Start setting up Repo indexer...");
    let repo_indexer_start_time = Instant::now();
    let ignore_checker = IgnoreChecker::new(&config.repo_path);
    let (mut watcher, indexer) = init_watcher_and_indexer(WatcherConfig {
        debounce_seconds: 10,
    });
    indexer.start();
    watcher
        .start_watch(Path::new(&config.repo_path), ignore_checker)
        .expect("Unable to start the watch!");
    info!(
        "Setting up the repo watcher complete. Took {}s",
        Instant::now()
            .duration_since(repo_indexer_start_time)
            .as_secs_f64()
    );

    let server_init_start_time = Instant::now();
    info!("Start building the server context...");

    let context = ServerContext::new(&config)?;

    let app = router(context);
    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()?;

    info!(
        "Initialization complete. Took {}s",
        Instant::now()
            .duration_since(server_init_start_time)
            .as_secs_f64()
    );

    info!("serving api at http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    serve(listener, app).await?;

    Ok(())
}
