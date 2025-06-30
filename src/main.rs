use std::io::{self, Write};

use indexer::Indexer;
use result_viewer::SearchResultViewer;
use searcher::Searcher;

mod git;
mod git_indexer;
mod index;
mod indexer;
mod result_viewer;
mod searcher;
mod tokenizer;

fn main() {
    let indexer = Indexer::new("/home/jaebum/learn/search");
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
