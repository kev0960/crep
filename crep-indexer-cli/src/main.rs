mod app;
mod searcher;

use std::{
    fs::File,
    io::{self},
    path::Path,
};

use app::App;
use clap::Parser;
use crep_indexer::index::{
    git_index::GitIndex,
    indexer::{IndexResult, Indexer, IndexerConfig},
};

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
}

fn main() -> io::Result<()> {
    color_eyre::install().unwrap();

    // let file = File::create("debug.log").unwrap();
    // let _redir = Redirect::stdout(file).unwrap();

    let mut terminal = ratatui::init();

    let args = Args::parse();
    let index = match args.load_path {
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
                    if let Some(save_path) = args.save_path {
                        index.save(std::path::Path::new(&save_path)).unwrap();
                    }

                    index
                }
                _ => panic!("Only Git index is supported"),
            }
        }
    };

    let searcher = Searcher::new(&index, &args.path);
    let result = App::new(searcher).run(&mut terminal);

    ratatui::restore();
    result
}
