mod app;
mod searcher;

use std::{
    io::{self, Write},
    path::Path,
};

use app::App;
use clap::Parser;
use crep_indexer::{
    index::{
        git_index::GitIndex,
        indexer::{IndexResult, Indexer, IndexerConfig},
    },
    search::{git_searcher::GitSearcher, result_viewer::GitSearchResultViewer},
};

use color_eyre::Result;
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};
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

    /*
    handle_query(index, &args.path);
    */

    let searcher = Searcher::new(&index);

    let result = App::new(
        args.load_path,
        &args.path,
        args.main_branch.as_deref(),
        args.save_path,
    )
    .run(&mut terminal);

    ratatui::restore();
    result
}

fn handle_query(index: GitIndex, path: &str) {
    let mut searcher = GitSearcher::new(&index);
    let viewer = GitSearchResultViewer::new(path, &index);

    loop {
        print!("Query :: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();

        if input.is_empty() {
            break;
        }

        let results = searcher.regex_search(input).unwrap();
        viewer.show_results(&results).unwrap();
    }
}
