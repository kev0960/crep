use std::io::{self, Write};

use anyhow::{Context, Result};
use crep_server::api::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<()> {
    let output = {
        let spec = ApiDoc::openapi();
        serde_json::to_string_pretty(&spec)
            .context("failed to serialise OpenAPI document to JSON")?
    };

    let mut stdout = io::stdout().lock();
    stdout
        .write_all(output.as_bytes())
        .context("failed to write output to stdout")?;

    Ok(())
}
