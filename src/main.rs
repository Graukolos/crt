#![warn(clippy::pedantic)]

mod ast;
mod codegen;
mod convert;
mod ffi;
mod network_ffi;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;

use crate::ast::Item;
use crate::codegen::{Backend, Program};

#[derive(Parser)]
struct Cli {
    xdf: PathBuf,
    source_dir: PathBuf,
    #[arg(long, default_value = "generated")]
    out: PathBuf,
    #[arg(long, value_enum, default_value_t = Backend::Naive)]
    backend: Backend,
    #[arg(long)]
    native_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 1024)]
    cap: usize,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Cli::parse();

    let network = match network_ffi::ffi::read_network(
        &args.xdf.to_string_lossy(),
        &args.source_dir.to_string_lossy(),
    ) {
        Ok(net) => net,
        Err(e) => {
            eprintln!("network read error: {e}");
            std::process::exit(1);
        }
    };

    let mut actors = HashMap::new();
    let mut units = Vec::new();
    let mut import_paths: HashSet<String> = HashSet::new();
    for class in &network.class_paths {
        let code = std::fs::read_to_string(&class.path)?;
        let raw = match ffi::ffi::parse_cal(&code) {
            Ok(ast) => ast,
            Err(e) => {
                eprintln!("parse error in {}: {e}", class.path);
                std::process::exit(1);
            }
        };
        let ast = convert::convert(&raw);
        import_paths.extend(ast.imports.into_iter().map(|i| i.path));
        match ast.item {
            Item::Actor(actor) => {
                actors.insert(class.class_name.clone(), actor);
            }
            Item::Unit(unit) => {
                units.push(unit);
            }
        }
    }

    let mut seen_imports: HashSet<PathBuf> = HashSet::new();
    for import_path in import_paths {
        let Some(file) = resolve_import_file(&args.source_dir, &import_path) else {
            continue;
        };
        if !seen_imports.insert(file.clone()) {
            continue;
        }
        let code = std::fs::read_to_string(&file)?;
        let raw = match ffi::ffi::parse_cal(&code) {
            Ok(ast) => ast,
            Err(e) => {
                eprintln!("warning: skipping imported {}: parse error: {e}", file.display());
                continue;
            }
        };
        if let Item::Unit(unit) = convert::convert(&raw).item {
            units.push(unit);
        }
    }

    let generator = args.backend.generator(args.cap);

    let native_dir = args.native_dir.or_else(|| {
        let convention = args.source_dir.join("..").join("lib").join("native");
        convention.is_dir().then_some(convention)
    });
    let native_sources = native_dir
        .as_deref()
        .map(discover_native_sources)
        .unwrap_or_default();
    if !native_sources.is_empty() {
        eprintln!(
            "discovered {} native source file(s) in {}",
            native_sources.len(),
            native_dir.as_ref().unwrap().display(),
        );
    }

    let program = Program {
        network: &network,
        actors: &actors,
        units: &units,
        native_sources: &native_sources,
    };
    generator.generate(&program, &args.out)?;
    eprintln!(
        "generated {} program for network '{}' in {}",
        generator.name(),
        network.name,
        args.out.display(),
    );

    Ok(())
}

fn resolve_import_file(source_dir: &Path, import_path: &str) -> Option<PathBuf> {
    let as_file = |dotted: &str| -> PathBuf {
        source_dir
            .join(dotted.split('.').collect::<PathBuf>())
            .with_extension("cal")
    };

    let full = as_file(import_path);
    if full.is_file() {
        return Some(full);
    }
    if let Some((package, _symbol)) = import_path.rsplit_once('.') {
        let file = as_file(package);
        if file.is_file() {
            return Some(file);
        }
    }
    None
}

fn discover_native_sources(dir: &std::path::Path) -> Vec<PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hh" | "hxx"
                )
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out
}
