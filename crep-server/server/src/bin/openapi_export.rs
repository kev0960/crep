use std::io::Write;
use std::io::{self};

use anyhow::Context;
use anyhow::Result;
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
