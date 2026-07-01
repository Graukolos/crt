use std::path::Path;

fn main() {
    let dcg = Path::new("Dataflow_Code_Generator");

    cxx_build::bridges(["src/ffi.rs", "src/network_ffi.rs"])
        .file("cpp/shim.cpp")
        .file("cpp/network_shim.cpp")
        .file(dcg.join("Lexer/Lexer.cpp"))
        .file(dcg.join("Parser/Parser.cpp"))
        .file(dcg.join("IR/AST/AST_Builder.cpp"))
        .file(dcg.join("Reader/Network_Reader.cpp"))
        .include(dcg)
        .include(dcg.join("common/include"))
        .include(".")
        .std("c++20")
        .warnings(false)
        .compile("crt_parser");

    for f in [
        "src/ffi.rs",
        "src/network_ffi.rs",
        "cpp/shim.cpp",
        "cpp/shim.hpp",
        "cpp/network_shim.cpp",
        "cpp/network_shim.hpp",
    ] {
        println!("cargo:rerun-if-changed={f}");
    }

    for f in [
        "Lexer/Lexer.cpp",
        "Parser/Parser.cpp",
        "IR/AST/AST_Builder.cpp",
        "Reader/Network_Reader.cpp",
    ] {
        println!("cargo:rerun-if-changed={}", dcg.join(f).display());
    }
}
