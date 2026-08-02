use std::fs;

pub fn list_books() -> std::io::Result<Vec<String>> {
    let mut books: Vec<String> = Vec::new();
    for entry in fs::read_dir("./books/")? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = path.file_name() {
            let name = name.to_os_string().into_string().unwrap();
            let split: Vec<&str> = name.split(".").collect();
            books.push(split[0].to_string());
        };
    }

    Ok(books)
}

pub fn file_path(book: String) -> String {
    let path = "~/workspaces/github/CodyBense/rupub/books/";
    let formatted = format!("{}{}", path, book);
    formatted
}
