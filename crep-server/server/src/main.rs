use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::serve;
use clap::Parser;
use crep_indexer::index::git_index_serialization::GitIndexSerialization;
use crep_indexer::index::git_indexer::GitIndexer;
use crep_indexer::index::git_indexer::GitIndexerConfig;
use crep_server::config::LiveIndexConfig;
use crep_server::config::ServerConfig;
use crep_server::indexer::indexer::Indexer;
use crep_server::reindex_notify::reindex_signal::ReindexSignal;
use crep_server::reindex_notify::repo_watcher::RepoWatcher;
use crep_server::router;
use crep_server::server_context::ServerContext;
use tokio::net::TcpListener;
use tokio::sync::mpsc::unbounded_channel;
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

    let serialized =
        GitIndexSerialization::load(&PathBuf::from(&config.saved_index_path))?;

    let index_config = GitIndexerConfig {
        show_index_progress: false,
        main_branch_name: "main".to_owned(),
        ignore_utf8_error: true,
    };

    let (send_indexer_signal, recv_indexer_signal) =
        unbounded_channel::<ReindexSignal>();
    let git_indexer = GitIndexer::from_saved(serialized, index_config);

    let mut _repo_watcher: Option<RepoWatcher> = None;
    if let Some(LiveIndexConfig::WatchLiveUpdate(_)) = &config.live_index_config
    {
        _repo_watcher =
            Some(RepoWatcher::new(&config, send_indexer_signal.clone())?);
    }

    info!(
        "Setting up the repo watcher complete. Took {}s",
        Instant::now()
            .duration_since(repo_indexer_start_time)
            .as_secs_f64()
    );

    let server_init_start_time = Instant::now();
    info!("Start building the server context...");

    let indexer = Arc::new(Indexer::new(
        git_indexer,
        &PathBuf::from(config.repo_path.clone()),
        send_indexer_signal,
    ));

    let context = ServerContext::new(&config, indexer.clone())?;

    indexer.spawn_re_indexer(recv_indexer_signal, context.search_cache.clone());

    let app = router(context, &config);
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
