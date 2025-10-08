mod app;
mod logger;
mod raw_searcher;
mod searcher;

use std::io::{self};
use std::path::Path;

use app::App;
use clap::Parser;
use crep_indexer::index::git_index::GitIndex;
use crep_indexer::index::indexer::IndexResult;
use crep_indexer::index::indexer::Indexer;
use crep_indexer::index::indexer::IndexerConfig;

use log::{LevelFilter, debug};
use logger::init_file_logger;
use raw_searcher::handle_query;
use searcher::Searcher;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to index.
    #[arg(short, long)]
    path: String,

    /// Main branch name
    #[arg(short, long)]
    main_branch: Option<String>,

    #[arg(short, long)]
    load_path: Option<String>,

    #[arg(short, long)]
    save_path: Option<String>,

    #[arg(short, long)]
    debug: bool,

    #[arg(long)]
    log: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let index = build_index(&args);
    let mut searcher = Searcher::new(&index, &args.path);

    println!("Debug {}", args.debug);
    if args.debug {
        env_logger::init();

        handle_query(&mut searcher).unwrap();

        Ok(())
    } else {
        if let Some(log) = args.log {
            init_file_logger(&log, LevelFilter::Debug).unwrap();
        }

        let mut terminal = ratatui::init();
        terminal.clear().unwrap();

        let result = App::new(searcher).run(&mut terminal);

        ratatui::restore();
        result
    }
}

fn build_index(args: &Args) -> GitIndex {
    match &args.load_path {
        Some(load_path) => GitIndex::load(Path::new(&load_path)).unwrap(),
        _ => {
            let indexer = Indexer::new(&IndexerConfig {
                root_dir: &args.path,
                main_branch_name: args.main_branch.as_deref(),
                ignore_utf8_error: true,
            });

            let index = indexer.index().unwrap();

            match index {
                IndexResult::GitIndex(index) => {
                    if let Some(save_path) = &args.save_path {
                        index.save(std::path::Path::new(&save_path)).unwrap();
                    }

                    index
                }
                _ => panic!("Only Git index is supported"),
            }
        }
    }
}
