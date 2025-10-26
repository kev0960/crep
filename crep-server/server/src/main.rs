use std::net::SocketAddr;

use axum::serve;
use clap::Parser;
use crep_server::config::ServerConfig;
use crep_server::router;
use crep_server::server_context::ServerContext;
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
    let context = ServerContext::new(&config)?;

    let app = router(context);
    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse()?;

    info!("serving api at http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    serve(listener, app).await?;

    Ok(())
}
