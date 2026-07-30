pub mod epub {
    use epub::doc::EpubDoc;
    use scraper::{Html, Selector};
    use std::{fs::File, io::BufReader};

    pub fn open_book(path: &str) -> EpubDoc<BufReader<File>> {
        EpubDoc::new(path).unwrap()
    }

    pub fn get_chapter_content(doc: &mut EpubDoc<BufReader<File>>) -> String {
        doc.get_current_str().unwrap().0
    }

    pub fn parse_chapter_content(content: String) -> String {
        let mut text = String::new();
        let fragment = Html::parse_document(&content);
        let selector = Selector::parse("p").unwrap();
        for element in fragment.select(&selector) {
            let collected: String = element.text().collect();
            text.push_str(collected.as_str());
            text.push_str("\n");
        }

        text
    }
}
