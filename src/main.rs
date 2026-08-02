mod book;
mod files;
use epub::doc::EpubDoc;
use scraper::{Html, Selector};
use std::{
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
    rc::Rc,
    slice::from_raw_parts,
    vec,
};

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    self, DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, List, ListDirection, ListState, Paragraph},
};

struct App {
    books: Vec<String>,
    list_state: ListState,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        App {
            books: fill_list().expect("No Books"),
            list_state,
        }
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.books.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.books.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn selected_book(&mut self) -> Option<&String> {
        self.list_state.selected().map(|i| &self.books[i])
    }
}

fn render_top_layer(frame: &mut Frame, outer_layout: &Rc<[Rect]>) {
    let top_layer = Layout::default()
        .margin(0)
        .constraints(vec![Constraint::Fill(1)])
        .split(outer_layout[0]);
    frame.render_widget(
        Paragraph::new("Enjoy reading your books in the terminal").block(
            Block::new()
                .title("Rupub")
                .bold()
                .fg(Color::Red)
                .borders(Borders::ALL),
        ),
        top_layer[0],
    );
}

fn render_middle_layer(frame: &mut Frame, outer_layout: &Rc<[Rect]>, app: &mut App) {
    let middle_layout = Layout::default()
        .direction(Direction::Horizontal)
        .margin(0)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer_layout[1]);

    frame.render_stateful_widget(
        List::new(app.books.clone())
            .block(
                Block::new()
                    .title("Book List")
                    .bold()
                    .fg(Color::Blue)
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::new().italic().fg(Color::LightGreen))
            .highlight_symbol(">> ")
            .repeat_highlight_symbol(true)
            .direction(ListDirection::TopToBottom),
        middle_layout[0],
        &mut app.list_state,
    );

    let preview_text = app
        .selected_book()
        .map(|book| format!("Selected: {}\n\nBook content would go here...", book))
        .unwrap_or_else(|| "No book selected".to_string());

    frame.render_widget(
        Paragraph::new(preview_text).block(
            Block::new()
                .title("Preview")
                .bold()
                .fg(Color::Blue)
                .borders(Borders::ALL),
        ),
        middle_layout[1],
    );
}

fn render_bottom_layer(frame: &mut Frame, outer_layout: &Rc<[Rect]>) {
    let bottom_layer = Layout::default()
        .margin(0)
        .constraints(vec![Constraint::Fill(1)])
        .split(outer_layout[2]);
    frame.render_widget(
        Paragraph::new("(q)uit | (j/k) up/down | (Enter) select").block(
            Block::new()
                .title("Key binds")
                .bold()
                .fg(Color::Red)
                .borders(Borders::ALL),
        ),
        bottom_layer[0],
    );
}

fn fill_list() -> std::io::Result<Vec<String>> {
    let books = files::list_books().unwrap();
    Ok(books)
}

// fn main() -> Result<()> {
//     color_eyre::install()?;
//     let terminal = ratatui::init();
//     let result = run(terminal);
//     ratatui::restore();
//     result
// }

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|frame| render(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Enter => {
                    if let Some(book) = app.selected_book() {
                        // open book view here
                    }
                }
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(vec![
            Constraint::Fill(1),
            Constraint::Percentage(85),
            Constraint::Fill(1),
        ])
        .split(frame.area());

    render_top_layer(frame, &outer_layout);
    render_middle_layer(frame, &outer_layout, app);
    render_bottom_layer(frame, &outer_layout);
}

fn main() {
    let books = files::list_books().unwrap();

    for book in books {
        println!("{}", book);
    }
    let mut doc = book::open_book("./src/test.epub");

    book::get_cover(&mut doc);
}
