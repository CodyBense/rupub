// TODO implemnt text wrap,change to scolling

mod book;
mod files;
use epub::doc::EpubDoc;
use image::DynamicImage;
use ratatui_image::{
    StatefulImage,
    picker::Picker,
    protocol::{Protocol, StatefulProtocol},
};
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
    image: StatefulProtocol,
    picker: Picker,
    layout_state: LayoutState,
    doc: Option<EpubDoc<BufReader<File>>>,
}

enum LayoutState {
    List,
    Reader,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let image = create_image(&picker).unwrap();

        App {
            books: fill_list().expect("No Books"),
            list_state,
            image,
            picker,
            layout_state: LayoutState::List,
            doc: None,
        }
    }

    fn refresh_cover(&mut self) {
        self.image = create_image(&self.picker).expect("cover to exist");
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

    frame.render_stateful_widget(StatefulImage::default(), middle_layout[1], &mut app.image);
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

fn render_reader_layer(frame: &mut Frame, reader_layer: &Rc<[Rect]>, app: &mut App) {
    let chapter_text =
        book::parse_chapter_content(book::get_chapter_content(app.doc.as_mut().unwrap()));
    let reader_layer = Layout::default()
        .margin(0)
        .constraints(vec![Constraint::Fill(1)]);
    frame.render_widget(
        Paragraph::new(chapter_text).block(Block::new().title("{}").bold().borders(Borders::ALL)),
        frame.area(),
    );
}

fn fill_list() -> std::io::Result<Vec<String>> {
    let books = files::list_books().unwrap();
    Ok(books)
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|frame| render(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => match app.layout_state {
                    LayoutState::List => break Ok(()),
                    LayoutState::Reader => app.layout_state = LayoutState::List,
                },
                KeyCode::Down | KeyCode::Char('j') => {
                    app.next();
                    create_cover(app.selected_book().unwrap());
                    app.refresh_cover();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.previous();
                    create_cover(app.selected_book().unwrap());
                    app.refresh_cover();
                }
                KeyCode::Enter => {
                    if let Some(book) = app.selected_book().cloned() {
                        app.layout_state = LayoutState::Reader;
                        let path = format!(
                            "/home/cody/workspaces/github/CodyBense/rupub/books/{}.epub",
                            book
                        );
                        app.doc = Some(book::open_book(path.as_str()));
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if let LayoutState::Reader = app.layout_state {
                        if let Some(doc) = app.doc.as_mut() {
                            doc.go_prev();
                        };
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if let LayoutState::Reader = app.layout_state {
                        if let Some(doc) = app.doc.as_mut() {
                            doc.go_next();
                        };
                    };
                }
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    match app.layout_state {
        LayoutState::List => {
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
        LayoutState::Reader => {
            // let outer_layout = Layout::default()
            //     .direction(Direction::Vertical)
            //     .margin(0)
            //     .constraints(vec![
            //         Constraint::Fill(1),
            //         Constraint::Percentage(85),
            //         Constraint::Fill(1),
            //     ])
            //     .split(frame.area());

            // render_top_layer(frame, &outer_layout);
            let reader_layer = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(vec![Constraint::Fill(1)])
                .split(frame.area());
            render_reader_layer(frame, &reader_layer, app);
        }
    }
}

fn create_image(picker: &Picker) -> Result<StatefulProtocol> {
    let img = image::ImageReader::open("/tmp/cover.jpeg")?.decode()?;
    let image_state = picker.new_resize_protocol(img);
    Ok(image_state)
}

fn create_cover(book_name: &String) {
    let path = format!(
        "/home/cody/workspaces/github/CodyBense/rupub/books/{}.epub",
        book_name
    );
    let mut doc = book::open_book(path.as_str());
    book::get_cover(&mut doc);
}
