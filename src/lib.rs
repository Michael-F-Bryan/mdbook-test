extern crate failure;
#[macro_use]
extern crate log;
extern crate mdbook;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[cfg(test)]
extern crate tempdir;
extern crate toml;

use std::path::Path;
use std::process::{Command, Stdio};
use std::fs::{self, File};
use std::io::{Read, Write};
use failure::{Error, ResultExt};
use mdbook::renderer::{RenderContext, Renderer};
use mdbook::book::{Book, BookItem, MDBook};
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
    info!("Starting Test");

    let crate_dir = &ctx.destination;

    let cfg = parse_config(ctx)?;
    let crate_name = sanitize_crate_name(ctx);

    debug!(
        "Creating test crate ({}) in {}",
        crate_name,
        crate_dir.display()
    );

    // there's no need to re-init if we ran previously
    if !crate_dir.join("Cargo.toml").exists() {
        create_crate(crate_dir, &crate_name, &cfg)?;
    }

    copy_across_book_chapters(&ctx.book, crate_dir)?;
    write_crate_contents(&cfg, &ctx.book, crate_dir)?;
    compile_and_test(crate_dir, &cfg)?;

    Ok(())
}

fn parse_config(ctx: &RenderContext) -> Result<Config, Error> {
    let cfg = match ctx.config.get("output.test") {
        Some(raw) => raw.clone().try_into()?,
        None => Config::default(),
    };

    if log_enabled!(::log::LogLevel::Debug) {
        for line in format!("{:#?}", cfg).lines() {
            debug!("{}", line);
        }
    }

    Ok(cfg)
}

fn sanitize_crate_name(ctx: &RenderContext) -> String {
    ctx.config
        .book
        .title
        .as_ref()
        .map(String::as_str)
        .unwrap_or("test_crate")
        .to_lowercase()
        .replace(" ", "_")
        .replace("-", "_")
}

fn create_crate(dir: &Path, name: &str, cfg: &Config) -> Result<(), Error> {
    debug!("Initializing crate");
    let mut cmd = Command::new("cargo");

    cmd.arg("init")
        .arg("--lib")
        .arg("--name")
        .arg(name)
        .env_remove("RUST_LOG");

    if cfg.quiet {
        cmd.arg("--quiet");
    }

    let status = cmd.arg(dir)
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
    debug!("Copying across book chapters");
    let src = dir.join("src");

    let chapters = book.sections.iter().filter_map(|b| match *b {
        BookItem::Chapter(ref ch) => Some(ch),
        _ => None,
    });

    for ch in chapters {
        debug!("Copying across {} ({})", ch.name, ch.path.display());
        let filename = src.join(&ch.path);

        if let Some(parent) = filename.parent() {
            fs::create_dir_all(parent)?;
        }

        File::create(filename)
            .and_then(|mut f| f.write_all(ch.content.as_bytes()))
            .with_context(|_| format!("Unable to copy across {}", ch.path.display()))?;
    }

    Ok(())
}

/// Generates the `build.rs` build script and adds the dependencies to
/// `Cargo.toml`.
fn write_crate_contents(cfg: &Config, book: &Book, dir: &Path) -> Result<(), Error> {
    debug!("Updating Cargo.toml");
    let cargo_toml_path = dir.join("Cargo.toml");

    let cargo_toml = load_toml(&cargo_toml_path)?;
    let updated_cargo_toml = update_cargo_toml(cargo_toml, &cfg.dependencies)?;
    dump_toml(&updated_cargo_toml, &cargo_toml_path)?;

    debug!("Writing build.rs");
    build_rs(book, dir.join("build.rs")).context("Unable to generate build.rs")?;

    debug!("Including `skeptic-tests.rs` in `lib.rs`");
    let mut lib_rs = File::create(dir.join("src").join("lib.rs")).context("Unable to open lib.rs")?;

    writeln!(
        lib_rs,
        r#"#[cfg(test)] include!(concat!(env!("OUT_DIR"), "/skeptic-tests.rs"));"#
    ).context("Couldn't write to lib.rs")?;

    Ok(())
}

fn build_rs<P: AsRef<Path>>(book: &Book, filename: P) -> Result<(), Error> {
    let mut f = File::create(filename)?;

    let template = include_str!("build_template.rs");

    let src = Path::new("src");

    let chapters = book.iter()
        .filter_map(|it| match *it {
            BookItem::Chapter(ref ch) => Some(ch),
            _ => None,
        })
        .map(|ch| src.join(&ch.path).display().to_string())
        .map(|ch| format!("r\"{}\"", ch))
        .collect::<Vec<_>>()
        .join(", ");

    let content = template.replace("$DEPS", &chapters);

    f.write_all(content.as_bytes())?;

    Ok(())
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
    // TODO: pull this out into a helper function
    value
        .entry("package".to_string())
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("unreachable")
        .insert(String::from("build"), "build.rs".into());

    value
        .entry("dev-dependencies".to_string())
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("unreachable")
        .insert(String::from("skeptic"), "*".into());

    value
        .entry("build-dependencies".to_string())
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("unreachable")
        .insert(String::from("skeptic"), "*".into());

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

fn compile_and_test(dir: &Path, cfg: &Config) -> Result<(), Error> {
    debug!("Compile and test ({})", dir.display());

    let mut cmd = Command::new("cargo");
    cmd.arg("test").env_remove("RUST_LOG");

    if cfg.quiet {
        cmd.arg("--quiet");
    }

    let status = cmd.current_dir(dir)
        .stdin(Stdio::null())
        .status()
        .context("Unable to invoke cargo")?;

    if let Some(code) = status.code() {
        debug!("Tests ran with return code {}", code);
    }

    if !status.success() {
        Err(failure::err_msg("The tests failed"))
    } else {
        Ok(())
    }
}

/// The configuration struct loaded from the `output.test` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    pub dependencies: Vec<String>,
    pub quiet: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            dependencies: Vec::new(),
            quiet: true,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct TestRenderer;

impl Renderer for TestRenderer {
    fn name(&self) -> &str {
        "test_renderer"
    }

    fn render(&self, md: &MDBook) -> Result<(), mdbook::errors::Error> {
        let root = md.root.clone();
        let dest = root.join(&md.config.build.build_dir);

        let ctx = RenderContext {
            version: MDBOOK_VERSION.to_string(),
            book: md.book.clone(),
            config: md.config.clone(),
            destination: dest,
            root: root,
        };

        if let Err(e) = test(&ctx) {
            warn!("Error: {}", e);
            for cause in e.causes().skip(1) {
                warn!("\tCaused By: {}", cause);
            }

            Err(format!("Testing Failed, {}", e).into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;
    use std::collections::HashSet;

    #[test]
    fn create_the_test_crate() {
        let temp = TempDir::new("mdbook-test").unwrap();
        let name = "test_crate";

        let cargo_toml = temp.path().join("Cargo.toml");

        assert!(!cargo_toml.exists());
        create_crate(temp.path(), name, &Default::default()).unwrap();
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
