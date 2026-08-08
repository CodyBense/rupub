// TODO implemnt text wrap,change to scolling

mod book;
mod files;
use epub::doc::EpubDoc;
use image::DynamicImage;
use ratatui_image::{
    Resize, StatefulImage,
    picker::Picker,
    protocol::{Protocol, StatefulProtocol},
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
};
use scraper::{Html, Selector};
use std::{
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
    rc::Rc,
    slice::from_raw_parts,
    thread, vec,
};
use std::{
    os::raw,
    sync::mpsc::{self, Receiver, Sender},
};

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    self, DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, List, ListDirection, ListState, Paragraph, Wrap},
};

struct App {
    books: Vec<String>,
    list_state: ListState,
    image: ThreadProtocol,
    picker: Picker,
    layout_state: LayoutState,
    doc: Option<EpubDoc<BufReader<File>>>,
    chapter_text: String,
    scroll_offset: u16,
    resize_tx: Sender<ResizeRequest>,
    cover_tx: Sender<String>,
}

enum LayoutState {
    List,
    Reader,
}

enum AppEvent {
    Term(Event),
    CoverReady(ResizeResponse),
    CoverLoaded(StatefulProtocol),
}

impl App {
    fn new(resize_tx: Sender<ResizeRequest>, cover_tx: Sender<String>) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        let raw = create_image(&picker).unwrap();
        let image = ThreadProtocol::new(resize_tx.clone(), Some(raw));

        App {
            books: fill_list().expect("No Books"),
            list_state,
            image,
            picker,
            layout_state: LayoutState::List,
            doc: None,
            chapter_text: String::new(),
            scroll_offset: 0,
            resize_tx,
            cover_tx,
        }
    }

    fn refresh_cover(&mut self) {
        if let Ok(raw) = create_image(&self.picker) {
            self.image = ThreadProtocol::new(self.resize_tx.clone(), Some(raw))
        }
    }

    fn refresh_chapter(&mut self) {
        if let Some(doc) = self.doc.as_mut() {
            self.chapter_text = book::parse_chapter_content(book::get_chapter_content(doc));
        };
        self.scroll_offset = 0;
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

    frame.render_stateful_widget(
        StatefulImage::default().resize(Resize::Scale((None))),
        middle_layout[1],
        &mut app.image,
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

fn render_reader_layer(frame: &mut Frame, reader_layer: &Rc<[Rect]>, app: &mut App) {
    let paragraph = Paragraph::new(app.chapter_text.clone())
        .scroll((app.scroll_offset, 0))
        .wrap(Wrap { trim: true })
        .block(Block::new().title("{}").bold().borders(Borders::ALL));

    frame.render_widget(paragraph, frame.area());
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
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();
    let (resize_tx, resize_rx) = mpsc::channel::<ResizeRequest>();
    let (cover_tx, cover_rx) = mpsc::channel::<String>();

    // Forward terminal input onto the shared event channel.
    {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            loop {
                match event::read() {
                    Ok(ev) => {
                        if event_tx.send(AppEvent::Term(ev)).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }

    // Background worker: does the actual resize+encode, off the UI thread.
    {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            while let Ok(request) = resize_rx.recv() {
                let response = request.resize_encode();
                if event_tx
                    .send(AppEvent::CoverReady(response.unwrap()))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    {
        let event_tx = event_tx.clone();
        let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
        thread::spawn(move || {
            while let Ok(book_name) = cover_rx.recv() {
                create_cover(&book_name);
                if let Ok(raw) = create_image(&picker) {
                    if event_tx.send(AppEvent::CoverLoaded(raw)).is_err() {
                        break;
                    }
                };
            }
        });
    }

    let mut app = App::new(resize_tx, cover_tx.clone());
    cover_tx.send(app.selected_book().unwrap().clone()).ok();

    loop {
        terminal.draw(|frame| render(frame, &mut app))?;

        match event_rx.recv() {
            Ok(AppEvent::CoverReady(response)) => {
                app.image.update_resized_protocol(response);
            }
            Ok(AppEvent::CoverLoaded(raw)) => {
                app.image = ThreadProtocol::new(app.resize_tx.clone(), Some(raw))
            }
            Ok(AppEvent::Term(Event::Key(key))) => match key.code {
                KeyCode::Char('q') => match app.layout_state {
                    LayoutState::List => return Ok(()),
                    LayoutState::Reader => app.layout_state = LayoutState::List,
                },
                KeyCode::Down | KeyCode::Char('j') => match app.layout_state {
                    LayoutState::List => {
                        app.next();
                        let book = app.selected_book().unwrap().clone();
                        app.cover_tx.send(book).ok();
                    }
                    LayoutState::Reader => {
                        app.scroll_offset = app.scroll_offset.saturating_add(1);
                    }
                },
                KeyCode::Up | KeyCode::Char('k') => match app.layout_state {
                    LayoutState::List => {
                        app.previous();
                        let book = app.selected_book().unwrap().clone();
                        app.cover_tx.send(book).ok();
                    }
                    LayoutState::Reader => {
                        app.scroll_offset = app.scroll_offset.saturating_sub(1);
                    }
                },
                KeyCode::Enter => {
                    if let Some(book) = app.selected_book().cloned() {
                        app.layout_state = LayoutState::Reader;
                        let path = format!(
                            "/home/cody/workspaces/github/CodyBense/rupub/books/{}.epub",
                            book
                        );
                        app.doc = Some(book::open_book(path.as_str()));
                        app.refresh_chapter();
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if let LayoutState::Reader = app.layout_state {
                        if let Some(doc) = app.doc.as_mut() {
                            doc.go_prev();
                        }
                        app.refresh_chapter();
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if let LayoutState::Reader = app.layout_state {
                        if let Some(doc) = app.doc.as_mut() {
                            doc.go_next();
                        }
                        app.refresh_chapter();
                    }
                }
                _ => {}
            },
            Ok(AppEvent::Term(_)) => {}
            Err(_) => return Ok(()),
        }
    }
    //     let mut app = App::new();

    //     loop {
    //         terminal.draw(|frame| render(frame, &mut app))?;

    //         if let Event::Key(key) = event::read()? {
    //             match key.code {
    //                 KeyCode::Char('q') => match app.layout_state {
    //                     LayoutState::List => break Ok(()),
    //                     LayoutState::Reader => app.layout_state = LayoutState::List,
    //                 },
    //                 KeyCode::Down | KeyCode::Char('j') => match app.layout_state {
    //                     LayoutState::List => {
    //                         app.next();
    //                         create_cover(app.selected_book().unwrap());
    //                         app.refresh_cover();
    //                     }
    //                     LayoutState::Reader => {
    //                         app.scroll_offset = app.scroll_offset.saturating_add(1);
    //                     }
    //                 },
    //                 KeyCode::Up | KeyCode::Char('k') => match app.layout_state {
    //                     LayoutState::List => {
    //                         app.previous();
    //                         create_cover(app.selected_book().unwrap());
    //                         app.refresh_cover();
    //                     }
    //                     LayoutState::Reader => {
    //                         app.scroll_offset = app.scroll_offset.saturating_sub(1);
    //                     }
    //                 },
    //                 KeyCode::Enter => {
    //                     if let Some(book) = app.selected_book().cloned() {
    //                         app.layout_state = LayoutState::Reader;
    //                         let path = format!(
    //                             "/home/cody/workspaces/github/CodyBense/rupub/books/{}.epub",
    //                             book
    //                         );
    //                         app.doc = Some(book::open_book(path.as_str()));
    //                         app.refresh_chapter();
    //                     }
    //                 }
    //                 KeyCode::Left | KeyCode::Char('h') => {
    //                     if let LayoutState::Reader = app.layout_state {
    //                         if let Some(doc) = app.doc.as_mut() {
    //                             doc.go_prev();
    //                         };
    //                         app.refresh_chapter();
    //                     }
    //                 }
    //                 KeyCode::Right | KeyCode::Char('l') => {
    //                     if let LayoutState::Reader = app.layout_state {
    //                         if let Some(doc) = app.doc.as_mut() {
    //                             doc.go_next();
    //                         };
    //                         app.refresh_chapter();
    //                     };
    //                 }
    //                 _ => {}
    //             }
    //         }
    //     }
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
