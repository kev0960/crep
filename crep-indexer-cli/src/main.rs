mod app;
mod logger;
mod raw_searcher;
mod searcher;

use std::io::{self};
use std::path::Path;

use app::App;
use clap::Parser;
use crep_indexer::index::git_index::GitIndex;
use crep_indexer::index::git_indexer::GitIndexer;
use crep_indexer::index::git_indexer::GitIndexerConfig;

use log::LevelFilter;
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

    #[arg(long)]
    save_only: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let index = build_index(&args);
    if args.save_only {
        return Ok(());
    }

    let mut searcher = Searcher::new(&index, &args.path);

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
            let mut indexer = GitIndexer::new(GitIndexerConfig {
                show_index_progress: true,
                main_branch_name: args
                    .main_branch
                    .as_deref()
                    .unwrap_or("main")
                    .to_owned(),
                ignore_utf8_error: true,
            });

            let repo = git2::Repository::open(Path::new(&args.path)).unwrap();
            indexer.index_history(repo).unwrap();

            let index = GitIndex::build(indexer);
            if let Some(save_path) = &args.save_path {
                index.save(std::path::Path::new(&save_path)).unwrap();
            }

            index
        }
    }
}
