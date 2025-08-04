use std::io::{self, Write};

use clap::Parser;
use index::indexer::Indexer;
use result_viewer::SearchResultViewer;
use searcher::Searcher;

mod git;
mod git_searcher;
mod index;
mod result_viewer;
mod searcher;
mod tokenizer;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to index.
    #[arg(short, long)]
    path: String,
}

fn main() {
    let args = Args::parse();

    let indexer = Indexer::new(&args.path);
    indexer.index();

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
