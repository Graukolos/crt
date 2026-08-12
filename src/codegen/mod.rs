mod common;
mod naive;
mod orcc;
mod rayon;
mod threads;
mod tokio;

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use proc_macro2::TokenStream;

use crate::ast::{Actor, NativeFunction, NativeProcedure, Unit};
use crate::codegen::common::{
    CHAN_MOD, actor_mod, chan_use, check_no_fanout, check_single_producer, emit_actor,
    emit_chan_file, emit_shared_decls,
};
use crate::network_ffi::ffi::Network;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    Naive,
    Threads,
    Rayon,
    Tokio,
}

#[derive(Copy, Clone)]
pub struct Options {
    pub cap: usize,
    pub fire_budget: usize,
    pub typestate: bool,
    pub orcc: bool,
}

impl Options {
    pub fn fire_budget_literal(self) -> String {
        if self.fire_budget == 0 {
            "usize::MAX".to_string()
        } else {
            self.fire_budget.to_string()
        }
    }
}

struct Spec {
    deps: String,
    chan_imports: &'static str,
    main_imports: &'static str,
    single_producer: bool,
    ports: fn(&Program<'_>, Options) -> String,
    actor_extra: fn(&Actor, Options) -> String,
    main: fn(&Program<'_>, Options) -> String,
}

fn no_actor_extra(_actor: &Actor, _options: Options) -> String {
    String::new()
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::Naive => "naive",
            Backend::Threads => "threads",
            Backend::Rayon => "rayon",
            Backend::Tokio => "tokio",
        }
    }

    fn spec(self) -> Spec {
        match self {
            Backend::Naive => Spec {
                deps: String::new(),
                chan_imports: common::LOCAL_CHAN_IMPORTS,
                main_imports: "use std::collections::VecDeque;\nuse std::rc::Rc;\n",
                single_producer: false,
                ports: common::local_ports,
                actor_extra: no_actor_extra,
                main: naive::emit_main,
            },
            Backend::Threads => Spec {
                deps: String::new(),
                chan_imports: common::RING_IMPORTS,
                main_imports: common::RING_MAIN_IMPORTS,
                single_producer: true,
                ports: common::ring_ports,
                actor_extra: no_actor_extra,
                main: threads::emit_main,
            },
            Backend::Rayon => Spec {
                deps: "rayon = \"1\"\n".to_string(),
                chan_imports: common::RING_IMPORTS,
                main_imports: common::RING_MAIN_IMPORTS,
                single_producer: true,
                ports: common::ring_ports,
                actor_extra: no_actor_extra,
                main: rayon::emit_main,
            },
            Backend::Tokio => Spec {
                deps: "tokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"sync\"] }\n"
                    .to_string(),
                chan_imports: "use std::collections::VecDeque;\n",
                main_imports: "use std::collections::VecDeque;\n",
                single_producer: false,
                ports: tokio::ports,
                actor_extra: tokio::actor_extra,
                main: tokio::emit_main,
            },
        }
    }
}

pub struct Program<'a> {
    pub network: &'a Network,
    pub actors: &'a BTreeMap<String, Box<Actor>>,
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

pub fn generate(
    backend: Backend,
    program: &Program<'_>,
    out_dir: &Path,
    options: Options,
) -> io::Result<()> {
    let spec = backend.spec();

    check_no_fanout(program)?;
    if spec.single_producer {
        check_single_producer(program)?;
    }

    let src_dir = out_dir.join("src");
    for (name, source) in emit_files(&spec, program, options) {
        let tokens = source.parse().map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "generated source for {name} failed to tokenize: {err}\n--- source ---\n{source}"
                ),
            )
        })?;
        write_rust(&src_dir.join(&name), tokens)?;
    }

    write_cargo_toml(
        out_dir,
        &program.network.name,
        program.has_natives(),
        &spec.deps,
        options.orcc,
    )?;
    if program.has_natives() {
        write_native_support(out_dir, program.native_sources, options.orcc)?;
    }
    Ok(())
}

fn emit_files(spec: &Spec, program: &Program<'_>, options: Options) -> Vec<(String, String)> {
    let mut files = Vec::new();

    for actor in program.actors.values() {
        let mut src = String::new();
        src.push_str("#![allow(warnings)]\n");
        src.push_str("use std::collections::VecDeque;\n");
        src.push_str("use super::*;\n\n");
        src.push_str(&emit_actor(actor, options.typestate));
        src.push_str(&(spec.actor_extra)(actor, options));
        files.push((format!("{}.rs", actor_mod(&actor.name)), src));
    }

    files.push((
        format!("{CHAN_MOD}.rs"),
        emit_chan_file(
            spec.chan_imports,
            options.cap,
            &(spec.ports)(program, options),
        ),
    ));

    let mut main = String::new();
    main.push_str("#![allow(warnings)]\n");
    main.push_str(spec.main_imports);
    main.push('\n');
    main.push_str(&chan_use());
    for actor in program.actors.values() {
        let _ = writeln!(main, "mod {};", actor_mod(&actor.name));
    }
    main.push('\n');
    let _ = writeln!(
        main,
        "const FIRE_BUDGET: usize = {};\n",
        options.fire_budget_literal()
    );
    main.push_str(&emit_shared_decls(program, options));
    main.push_str(&(spec.main)(program, options));
    files.push(("main.rs".to_string(), main));

    files
}

pub fn write_cargo_toml(
    out_dir: &Path,
    package_name: &str,
    has_natives: bool,
    extra_deps: &str,
    orcc: bool,
) -> io::Result<()> {
    let name = cargo_package_name(package_name);
    let mut contents = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n"
    );
    contents.push_str(extra_deps);
    if has_natives {
        if orcc {
            contents.push_str("clap = { version = \"4\", features = [\"derive\"] }\n");
        }
        contents.push_str("\n[build-dependencies]\ncc = \"1\"\n");
    }
    contents.push_str("\n[profile.release]\nlto = \"thin\"\npanic = \"abort\"\n");
    write_file(&out_dir.join("Cargo.toml"), &contents)
}

pub fn write_native_support(
    out_dir: &Path,
    native_sources: &[PathBuf],
    orcc: bool,
) -> io::Result<()> {
    let native_dir = out_dir.join("native");
    fs::create_dir_all(&native_dir)?;

    let mut translation_units = Vec::new();
    for src in native_sources {
        let Some(file_name) = src.file_name() else {
            continue;
        };
        fs::copy(src, native_dir.join(file_name))?;
        let name = file_name.to_string_lossy().to_string();
        if let Some(ext) = src.extension().and_then(|e| e.to_str())
            && matches!(
                ext.to_ascii_lowercase().as_str(),
                "c" | "cpp" | "cc" | "cxx"
            )
        {
            translation_units.push(name);
        }
    }

    if orcc {
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
