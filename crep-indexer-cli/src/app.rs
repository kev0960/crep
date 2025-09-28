use std::collections::BTreeMap;
use std::{io, sync::mpsc::channel};

use crep_indexer::search::search_result::SearchResult;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    widgets::{Block, Paragraph},
};
use std::fs::{File, OpenOptions};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use tui_input::{Input, backend::crossterm::EventHandler};

use crate::searcher::{Query, Searcher};

#[derive(PartialEq, Copy, Clone)]
pub enum State {
    Control,
    Input(QueryType),
    Terminate,
}

#[derive(PartialEq, Copy, Clone)]
pub enum QueryType {
    Regex,
    RawString,
}

pub struct App<'a> {
    state: RwLock<State>,
    searcher: Arc<Mutex<Searcher<'a>>>,
    input: Input,

    ui_send: mpsc::Sender<Message>,
    ui_recv: mpsc::Receiver<Message>,

    search_send: mpsc::Sender<SearchRequest>,
    search_recv: Option<mpsc::Receiver<SearchRequest>>,

    search_result: Vec<SearchResult>,
}

#[derive(Debug)]
enum Message {
    Event(Event),
    SearchResults(Vec<SearchResult>),
    Terminate,
}

struct SearchRequest {
    query: Query,
}

impl<'a> App<'a> {
    pub fn new(searcher: Searcher<'a>) -> Self {
        let (ui_send, ui_recv) = channel();
        let (search_send, search_recv) = channel();

        Self {
            state: RwLock::new(State::Input(QueryType::RawString)),
            input: Input::default(),
            searcher: Arc::new(Mutex::new(searcher)),
            ui_send,
            ui_recv,
            search_send,
            search_recv: Some(search_recv),
            search_result: vec![],
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        {
            // Create a thread that handles the user input.
            let ui_send = self.ui_send.clone();
            std::thread::spawn(move || {
                while let Ok(event) = event::read() {
                    ui_send.send(Message::Event(event)).unwrap()
                }
            });
        }

        std::thread::scope(|scope| {
            // Create another thread that handles the search request.
            let search_recv = self.search_recv.take().unwrap();
            let ui_send = self.ui_send.clone();

            let searcher = self.searcher.clone();

            scope.spawn(move || {
                while let Ok(first) = search_recv.recv() {
                    // Drain to get the last one.
                    let mut last = first;
                    while let Ok(m) = search_recv.try_recv() {
                        last = m;
                    }

                    let mut searcher = searcher.lock().unwrap();
                    if let Ok(search_results) =
                        searcher.handle_query(&last.query)
                    {
                        ui_send
                            .send(Message::SearchResults(search_results))
                            .unwrap();
                    }
                }
            });

            loop {
                {
                    let state = self.state.read().unwrap();
                    if *state == State::Terminate {
                        break;
                    }
                }

                terminal.draw(|frame| self.render(frame)).unwrap();

                if let Ok(message) = self.ui_recv.recv() {
                    match message {
                        Message::Event(e) => self.handle_event(e).unwrap(),
                        Message::SearchResults(results) => {
                            self.search_result = results;
                        }
                        Message::Terminate => break,
                    }
                } else {
                    break;
                }
            }
        });

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> io::Result<()> {
        let state = {
            let state = self.state.read().unwrap();
            *state
        };

        if let Event::Key(key_event) = event {
            if key_event.code == KeyCode::Esc {
                match state {
                    State::Control => {
                        *self.state.write().unwrap() = State::Terminate;
                        self.ui_send.send(Message::Terminate).unwrap();
                    }
                    State::Input(_) => {
                        *self.state.write().unwrap() = State::Control;
                    }
                    State::Terminate => { /* Ignore */ }
                }
            } else if key_event.code == KeyCode::Enter {
                if let State::Input(_) = state {
                    // Clear the input on enter.
                    self.input.reset();
                }
            } else {
                match state {
                    State::Control => {
                        if key_event.code == KeyCode::Char('i') {
                            *self.state.write().unwrap() =
                                State::Input(QueryType::RawString);
                        } else if key_event.code == KeyCode::Char('r') {
                            *self.state.write().unwrap() =
                                State::Input(QueryType::Regex);
                        }
                    }
                    State::Input(query_type) => {
                        self.input.handle_event(&event);
                        match query_type {
                            QueryType::Regex => {
                                self.search_send
                                    .send(SearchRequest {
                                        query: Query::Regex(
                                            self.input.value().to_owned(),
                                        ),
                                    })
                                    .unwrap();
                            }
                            QueryType::RawString => {
                                self.search_send
                                    .send(SearchRequest {
                                        query: Query::RawString(
                                            self.input.value().to_owned(),
                                        ),
                                    })
                                    .unwrap();
                            }
                        }
                    }
                    State::Terminate => { /* Ignore */ }
                }
            }
        }

        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let [header, input, search_results, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        frame.render_widget(
            Paragraph::new("Crep - CodeGrep".bold())
                .alignment(ratatui::layout::Alignment::Center),
            header,
        );

        self.render_input(frame, input);
        self.render_search_result(frame, search_results, &self.search_result);
        self.render_status(frame, status);
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let width = area.width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);
        let input = Paragraph::new(self.input.value())
            .scroll((0, scroll as u16))
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, area);

        // Ratatui hides the cursor unless it's explicitly set. Position the  cursor past the
        // end of the input text and one line down from the border to the input line
        let x = self.input.visual_cursor().max(scroll) - scroll + 1;
        frame.set_cursor_position((area.x + x as u16, area.y + 1))
    }

    fn render_search_result(
        &self,
        frame: &mut Frame,
        area: Rect,
        results: &[SearchResult],
    ) {
        let mut lines = vec![];
        for result in results {
            for (line, words) in &result.words_per_line {
                if words.is_empty() {
                    lines.push(Line::from(
                        result.lines.get(line).unwrap().as_str(),
                    ))
                } else {
                    let line = result.lines.get(line).unwrap().as_str();
                    lines.push(get_highlighted_line(line, words))
                }
            }
        }

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let state = self.state.read().unwrap();
        match *state {
            State::Control => {
                frame.render_widget(
                    Paragraph::new("Use ESC to terminate. i: String search. r: Regex search")
                        .style(Style::default().fg(Color::Yellow)),
                    area,
                );
            }
            State::Input(query_type) => {
                let text = match query_type {
                    QueryType::Regex => "Regex",
                    QueryType::RawString => "Text",
                };

                frame.render_widget(
                    Paragraph::new(format!("{text} - Use ESC to escape"))
                        .style(Style::default().fg(Color::Green)),
                    area,
                );
            }
            State::Terminate => {
                frame.render_widget(
                    Paragraph::new("Terminating...")
                        .style(Style::default().fg(Color::Red)),
                    area,
                );
            }
        }
    }
}

pub fn get_highlighted_line<'a>(
    line: &'a str,
    positions: &[(String, usize)],
) -> Line<'a> {
    let mut result = vec![];

    let mut current = 0;
    for pos in positions {
        let (word, start) = pos;

        if current < *start {
            result.push(Span::raw(truncate_long_line(&line[current..*start])));
        }

        result.push(Span::styled(
            word.to_string().red().to_string(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

        current = start + word.len();
    }

    if current < line.len() {
        result.push(Span::raw(truncate_long_line(&line[current..])));
    }

    Line::from(result)
}

fn truncate_long_line(line: &str) -> String {
    if line.len() < MAX_CHARS_TO_SHOW {
        return line.to_owned();
    }

    format!(
        "{} ... {}",
        get_first_n_chars(line, MAX_CHARS_TO_SHOW / 2),
        get_last_n_chars(line, MAX_CHARS_TO_SHOW / 2)
    )
}

fn get_first_n_chars(line: &str, n: usize) -> &str {
    let end_byte_index = line
        .char_indices()
        .nth(n)
        .map_or(line.len(), |(idx, _)| idx);

    &line[0..end_byte_index]
}

fn get_last_n_chars(line: &str, n: usize) -> &str {
    let start_byte_index = line
        .char_indices()
        .rev()
        .nth(n.saturating_sub(1))
        .map_or(0, |(idx, _)| idx);

    &line[start_byte_index..]
}

const MAX_CHARS_TO_SHOW: usize = 80;
