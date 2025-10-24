use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use crep_server::api::{self, ApiDoc};

fn main() -> Result<()> {
    let options = parse_args()?;
    let output = match options.format {
        OutputFormat::Json => {
            let spec = ApiDoc::openapi();
            serde_json::to_string_pretty(&spec)
                .context("failed to serialise OpenAPI document to JSON")?
        }
        OutputFormat::TypeScript => api::search::typescript_definitions(),
    };

    write_output(&options.out, &output)?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Json,
    TypeScript,
}

impl OutputFormat {
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "json" => Ok(Self::Json),
            "ts" | "typescript" => Ok(Self::TypeScript),
            other => Err(anyhow!(
                "unknown format '{other}'. Supported formats: json, ts"
            )),
        }
    }
}

struct Options {
    format: OutputFormat,
    out: Option<PathBuf>,
}

fn parse_args() -> Result<Options> {
    let mut args = env::args().skip(1);

    let mut format = OutputFormat::Json;
    let mut out = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("--format requires a value"))?;
                format = OutputFormat::from_str(&value)?;
            }
            "--out" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("--out requires a path"))?;
                out = Some(PathBuf::from(value));
            }
            other => {
                return Err(anyhow!(
                    "unrecognised argument '{other}'. Run with --help for usage."
                ));
            }
        }
    }

    Ok(Options { format, out })
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

fn print_help() {
    println!(
        "Export the crep-server OpenAPI description.\n\n\
         Usage: openapi-export [--format json|ts] [--out <path>]\n\n\
         Options:\n  \
         --format <json|ts>    Output format (default: json)\n  \
         --out <path>          Write to file instead of stdout\n  \
         -h, --help            Show this help text"
    );
}
