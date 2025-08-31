use std::{
    io::{self, Write},
    path::Path,
};

use clap::Parser;
use crep_indexer::{
    git_searcher::GitSearcher,
    index::{
        git_index::GitIndex,
        indexer::{IndexResult, Indexer, IndexerConfig},
    },
    search::result_viewer::GitSearchResultViewer,
};

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

fn main() {
    let args = Args::parse();

    let index = match args.load_path {
        Some(load_path) => GitIndex::load(Path::new(&load_path)).unwrap(),
        _ => {
            let indexer = Indexer::new(&IndexerConfig {
                root_dir: &args.path,
                main_branch_name: args.main_branch.as_deref(),
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

    handle_query(index, &args.path);
    /*
        let indexer = Indexer::new("/home/jaebum/Halfmore");
        let index = indexer.index_directory();

        let searcher = Searcher::new(&index);

        let mut result_viewer = SearchResultViewer::new();

        loop {
            print!("Query :: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            let input = input.trim();

            if input.is_empty() {
                break;
            }

            let results = searcher.search(input);
            println!(
                "{}",
                result_viewer.show_results(&results, &index.file_to_word_pos)
            );
        }
    */
}

fn handle_query(index: GitIndex, path: &str) {
    let searcher = GitSearcher::new(&index);
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

        let results = searcher.search(input);
        viewer.show_results(&results).unwrap();
    }
}
