extern crate mdbook;
extern crate env_logger;
extern crate mdbook_test;

use std::path::{Path, PathBuf};
use mdbook::renderer::RenderContext;
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::config::Config as MdConfig;
use mdbook_test::Config;

fn new_chapter<P: AsRef<Path>>(path: P) -> BookItem {
    let path = path.as_ref();
    let filename = path.file_name().and_then(|p| p.to_str()).unwrap();

    let ch = Chapter::new(filename, String::new(), path);

    BookItem::Chapter(ch)
}

fn create_context() -> RenderContext {
    let chapters = vec!["first.md", "second.md", "nested/third.md"];
    let mut book = Book::default();

    for ch in &chapters {
        book.sections.push(new_chapter(ch));
    }

    let cfg = Config {
        dependencies: Vec::new(),
        quiet: true,
    };

    let mut md_config = MdConfig::default();
    md_config.set("output.test", cfg).unwrap();

    let render_context = RenderContext {
        version: mdbook_test::MDBOOK_VERSION.to_string(),
        book: book,
        root: PathBuf::new(),
        config: md_config,
        destination: PathBuf::new(),
    };

    render_context 
}

#[test]
fn test_the_entire_process() {
    env_logger::init().ok();

    let ctx = create_context();

    macro_rules! unwrap {
            ($thing:expr) => {
                if let Err(e) = $thing {
                    println!("Error: {}", e);

                    for cause in e.causes().skip(1) {
                        println!("\tCaused By: {}", cause);
                    }

                    panic!("`{}` failed", stringify!($thing));
                }
            };
        }

    unwrap!(mdbook_test::test(&ctx));
}
