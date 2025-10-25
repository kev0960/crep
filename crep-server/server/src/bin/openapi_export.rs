use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use crep_server::api::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<()> {
    let options = Cli::parse();
    let output = {
        let spec = ApiDoc::openapi();
        serde_json::to_string_pretty(&spec)
            .context("failed to serialise OpenAPI document to JSON")?
    };

    write_output(&options.out, &output)?;

    Ok(())
}

#[derive(Parser)]
#[command(
    name = "openapi-export",
    about = "Export the crep-server OpenAPI description."
)]
struct Cli {
    /// Write to file instead of stdout
    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,
}

fn write_output(path: &Option<PathBuf>, contents: &str) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create directory {}", parent.display())
                })?;
            }
        }

        fs::write(path, contents).with_context(|| {
            format!("failed to write output to {}", path.display())
        })?;
        eprintln!("Wrote {}", path.display());
    } else {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(contents.as_bytes())
            .context("failed to write output to stdout")?;
    }

    Ok(())
}
