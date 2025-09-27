use std::{io, path::Path, sync::mpsc::channel};

use crep_indexer::{
    index::{
        git_index::GitIndex,
        indexer::{IndexResult, Indexer, IndexerConfig},
    },
    result_viewer::SearchResult,
};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph, Widget},
};
use std::sync::mpsc;
use tui_input::{Input, backend::crossterm::EventHandler};

pub struct App {
    index: GitIndex,
    exit: bool,
    input: Input,

    ui_send: mpsc::Sender<Message>,
    ui_recv: mpsc::Receiver<Message>,

    search_send: mpsc::Sender<SearchRequest>,
    search_recv: Option<mpsc::Receiver<SearchRequest>>,
}

enum Message {
    Event(Event),
    SearchResult(SearchResult),
}

struct SearchRequest {
    query: String,
    enqueue_timestamp: u64,
}

impl App {
    pub fn new() -> Self {
        let (ui_send, ui_recv) = channel();
        let (search_send, search_recv) = channel();

        Self {
            index,
            exit: false,
            input: Input::default(),
            ui_send,
            ui_recv,
            search_send,
            search_recv: Some(search_recv),
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

        {
            // Create another thread that handles the search request.
            let search_recv = self.search_recv.take().unwrap();
            std::thread::spawn(move || {
                while let Ok(first) = search_recv.recv() {
                    // Drain to get the last one.
                    let mut last = first;
                    while let Ok(m) = search_recv.try_recv() {
                        last = m;
                    }
                }
            });
        }

        while !self.exit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }

        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        let event = event::read()?;
        match event {
            Event::Key(key_event) => {
                if key_event.code == KeyCode::Esc {
                    self.exit();
                } else {
                    self.input.handle_event(&event);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true
    }

    fn render(&self, frame: &mut Frame) {
        let [header, input, search_results] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .areas(frame.area());

        frame.render_widget(
            Paragraph::new("Crep - CodeGrep".bold())
                .alignment(ratatui::layout::Alignment::Center),
            header,
        );

        self.render_input(frame, input);

        frame.render_widget(Paragraph::new(self.input.value()), search_results);
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
}
