extern crate failure;
extern crate mdbook;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate tempdir;

use std::path::Path;
use std::process::{Command, Stdio};
use tempdir::TempDir;
use failure::{Error, ResultExt, SyncFailure};
use mdbook::renderer::RenderContext;
use mdbook::book::Book;

/// Test each of the Rust snippets in a book.
///
/// This works by:
///
/// - Creating a new crate in a temporary directory
/// - Copying across the book chapters as `*.md` files
/// - Update `Cargo.toml` to include any extra dependencies the user specifies
/// - Run [rust-skeptic] over the generated crate
///
/// [rust-skeptic]: https://github.com/budziq/rust-skeptic
pub fn test(ctx: &RenderContext) -> Result<(), Error> {
    let cfg: Config = ctx.config
        .get_deserialized("output.test")
        .map_err(SyncFailure::new)?;

    let temp = TempDir::new("mdbook-test").context("Unable to create a temporary directory")?;
    let crate_dir = temp.path();

    let crate_name = ctx.config
        .book
        .title
        .as_ref()
        .map(String::as_str)
        .unwrap_or("mdbook_test");

    create_crate(crate_dir, crate_name)?;
    copy_across_book_chapters(&ctx.book, crate_dir)?;
    generate_build_rs(&ctx.book, &cfg, crate_dir)?;

    compile_and_test(crate_dir)?;

    Ok(())
}

fn create_crate(dir: &Path, name: &str) -> Result<(), Error> {
    let status = Command::new("cargo")
        .arg("init")
        .arg("--name")
        .arg(name)
        .arg(dir)
        .stdin(Stdio::null())
        .status()
        .context("Unable to invoke cargo")?;

    if !status.success() {
        Err(failure::err_msg("Couldn't initialize the testing crate"))
    } else {
        Ok(())
    }
}

fn copy_across_book_chapters(book: &Book, dir: &Path) -> Result<(), Error> {
    unimplemented!()
}

fn generate_build_rs(book: &Book, cfg: &Config, dir: &Path) -> Result<(), Error> {
    unimplemented!()
}

fn compile_and_test(dir: &Path) -> Result<(), Error> {
    unimplemented!()
}

/// The configuration struct loaded from the `output.test` table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    pub dependencies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_the_test_crate() {
        let temp = TempDir::new("mdbook-test").unwrap();
        let name = "test_crate";

        let cargo_toml = temp.path().join("Cargo.toml");

        assert!(!cargo_toml.exists());
        create_crate(temp.path(), name).unwrap();
        assert!(cargo_toml.exists());
    }
}
