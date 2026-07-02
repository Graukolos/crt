mod common;
mod naive;
mod orcc;
mod tokio;

use std::collections::HashMap;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use proc_macro2::TokenStream;

use crate::ast::{Actor, NativeFunction, NativeProcedure, Unit};
use crate::network_ffi::ffi::Network;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    Naive,
    Rayon,
    Tokio,
}

impl Backend {
    pub fn generator(self, cap: usize) -> Option<Box<dyn CodeGenerator>> {
        match self {
            Backend::Naive => Some(Box::new(naive::Naive)),
            Backend::Tokio => Some(Box::new(tokio::Tokio { cap })),
            Backend::Rayon => None,
        }
    }
}

pub struct Program<'a> {
    pub network: &'a Network,
    pub actors: &'a HashMap<String, Box<Actor>>,
    pub units: &'a [Unit],
    pub native_sources: &'a [PathBuf],
}

impl Program<'_> {
    pub fn has_natives(&self) -> bool {
        let uses_native = |fns: &[NativeFunction], procs: &[NativeProcedure]| {
            !fns.is_empty() || !procs.is_empty()
        };
        self.actors
            .values()
            .any(|a| uses_native(&a.native_functions, &a.native_procedures))
            || self
                .units
                .iter()
                .any(|u| uses_native(&u.native_functions, &u.native_procedures))
    }
}

pub trait CodeGenerator {
    fn name(&self) -> &'static str;

    fn generate(&self, program: &Program<'_>, out_dir: &Path) -> io::Result<()>;
}

pub fn write_cargo_toml(
    out_dir: &Path,
    package_name: &str,
    has_natives: bool,
    extra_deps: &str,
) -> io::Result<()> {
    let name = cargo_package_name(package_name);
    let mut contents = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n"
    );
    contents.push_str(extra_deps);
    if has_natives {
        contents.push_str("clap = { version = \"4\", features = [\"derive\"] }\n");
        contents.push_str("\n[build-dependencies]\ncc = \"1\"\n");
    }
    contents.push_str("\n[profile.release]\nlto = true\ncodegen-units = 1\n");
    write_file(&out_dir.join("Cargo.toml"), &contents)
}

pub fn write_native_support(out_dir: &Path, native_sources: &[PathBuf]) -> io::Result<()> {
    let native_dir = out_dir.join("native");
    fs::create_dir_all(&native_dir)?;

    let mut translation_units = Vec::new();
    let mut have_options_h = false;
    for src in native_sources {
        let Some(file_name) = src.file_name() else {
            continue;
        };
        fs::copy(src, native_dir.join(file_name))?;
        let name = file_name.to_string_lossy().to_string();
        if name == "options.h" {
            have_options_h = true;
        }
        if let Some(ext) = src.extension().and_then(|e| e.to_str())
            && matches!(
                ext.to_ascii_lowercase().as_str(),
                "c" | "cpp" | "cc" | "cxx"
            )
        {
            translation_units.push(name);
        }
    }

    if !have_options_h {
        write_file(&native_dir.join("options.h"), orcc::OPTIONS_H)?;
    }

    let files: String = translation_units
        .iter()
        .fold(String::new(), |mut output, name| {
            let _ = writeln!(output, "        .file(\"native/{name}\")");
            output
        });
    let build_rs = format!(
        "fn main() {{\n    \
         cc::Build::new()\n        \
         .flag(\"-x\")\n        .flag(\"c\")\n        \
         .include(\"native\")\n        .opt_level(3)\n{files}        \
         .compile(\"crt_native\");\n    \
         println!(\"cargo:rerun-if-changed=native\");\n}}\n"
    );
    write_file(&out_dir.join("build.rs"), &build_rs)
}

fn cargo_package_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

pub fn write_rust(path: &Path, tokens: TokenStream) -> io::Result<()> {
    let file = syn::parse2::<syn::File>(tokens).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("generated tokens are not a valid Rust file: {err}"),
        )
    })?;
    write_file(path, &prettyplease::unparse(&file))
}

pub fn write_file(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
