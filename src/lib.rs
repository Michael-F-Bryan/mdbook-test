extern crate failure;
extern crate mdbook;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate tempdir;
extern crate toml;

use std::path::Path;
use std::process::{Command, Stdio};
use std::fs::File;
use std::io::{Read, Write};
use tempdir::TempDir;
use failure::{Error, ResultExt, SyncFailure};
use mdbook::renderer::RenderContext;
use mdbook::book::Book;
use toml::value::{Table, Value};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// The exact version of `mdbook` this crate is compiled against.
pub const MDBOOK_VERSION: &'static str = env!("MDBOOK_VERSION");

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

/// Generates the `build.rs` build script and adds the dependencies to
/// `Cargo.toml`.
fn generate_build_rs(book: &Book, cfg: &Config, dir: &Path) -> Result<(), Error> {
    let cargo_toml_path = dir.join("Cargo.toml");

    let cargo_toml = load_toml(&cargo_toml_path)?;
    let updated_cargo_toml = update_cargo_toml(cargo_toml, &cfg.dependencies)?;
    dump_toml(&updated_cargo_toml, &cargo_toml_path)?;


    // TODO: Generate the build.rs
    unimplemented!()
}

fn dump_toml<P: AsRef<Path>, S: Serialize>(thing: &S, filename: P) -> Result<(), Error> {
    let filename = filename.as_ref();
    let as_str = toml::to_string(thing).context("Couldn't serialize toml")?;

    File::create(filename)
        .and_then(|mut f| f.write_all(as_str.as_bytes()))
        .with_context(|_| format!("Unable to save to {}", filename.display()))?;

    Ok(())
}

fn load_toml<P: AsRef<Path>, D: DeserializeOwned>(filename: P) -> Result<D, Error> {
    let mut contents = String::new();
    let filename = filename.as_ref();

    File::open(filename)
        .and_then(|mut f| f.read_to_string(&mut contents))
        .with_context(|_| format!("Unable to read {}", filename.display()))?;

    toml::from_str(&contents)
        .with_context(|_| format!("Couldn't to parse {}", filename.display()))
        .map_err(Error::from)
}

fn update_cargo_toml(mut value: Table, deps: &[String]) -> Result<Value, Error> {
    value
        .entry("package".to_string())
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("unreachable")
        .insert(String::from("build"), "build.rs".into());

    {
        let deps_table = value
            .entry("dependencies".to_string())
            .or_insert_with(|| Value::Table(Table::new()))
            .as_table_mut()
            .expect("unreachable");

        for dep in deps {
            deps_table.insert(dep.clone(), "*".into());
        }
    }

    Ok(Value::Table(value))
}

fn compile_and_test(dir: &Path) -> Result<(), Error> {
    let status = Command::new("cargo")
        .arg("test")
        .current_dir(dir)
        .stdin(Stdio::null())
        .status()
        .context("Unable to invoke cargo")?;

    if !status.success() {
        Err(failure::err_msg("The tests failed"))
    } else {
        Ok(())
    }
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
    use std::collections::HashSet;

    #[test]
    fn create_the_test_crate() {
        let temp = TempDir::new("mdbook-test").unwrap();
        let name = "test_crate";

        let cargo_toml = temp.path().join("Cargo.toml");

        assert!(!cargo_toml.exists());
        create_crate(temp.path(), name).unwrap();
        assert!(cargo_toml.exists());
    }

    fn get_build(table: &Table) -> Option<&Value> {
        table
            .get("package")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("build"))
    }

    #[test]
    fn build_rs_is_added_to_cargo_toml() {
        let original = Table::new();

        assert!(get_build(&original).is_none());

        let got = update_cargo_toml(original, &[]).unwrap();

        let got_as_table = got.as_table().unwrap();
        assert!(get_build(got_as_table).is_some());
    }

    #[test]
    fn dependencies_are_added_to_cargo_toml() {
        let original = Table::new();
        let deps = vec![
            String::from("foo"),
            String::from("bar"),
            String::from("baz"),
        ];

        let got = update_cargo_toml(original, &deps).unwrap();

        let got_deps: HashSet<_> = got.as_table()
            .and_then(|t| t.get("dependencies"))
            .and_then(Value::as_table)
            .unwrap()
            .keys()
            .collect();

        assert_eq!(got_deps.len(), deps.len());
        for dep in &deps {
            assert!(got_deps.contains(dep));
        }
    }
}
