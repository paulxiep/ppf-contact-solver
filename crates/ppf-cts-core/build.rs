// File: build.rs
// Code: Claude Code
// Review: Ryoichi Ando (ryoichi.ando@zozo.com)
// License: Apache v2.0
//
// Compiles the host-callable C ABI shim (cpp/intersect_ffi.cpp) over the
// shared edge-triangle pierce predicate that lives in the CUDA solver tree
// (../ppf-cts-solver/src/cpp/contact/intersect_core.hpp). This makes the
// device intersection predicate the single source of truth: the Rust
// build-time self-intersection check links this object and calls the same
// geometry the GPU kernels run, instead of a parallel Rust port.
//
// This is a plain host C++ compile (no nvcc, no CUDA): the predicate
// header is dependency-free and STL-free, so it also builds on a no-CUDA
// host (macOS emulated path) with the system C++ compiler.
//
// It also generates the cubin list behind `utils::SUPPORTED_SM` from the
// same `cuda_arch.txt` the two CUDA builds read, so the architectures the
// run-time gate accepts cannot differ from the ones actually linked.

use std::path::Path;

/// Architectures the solver ships a cubin for, read from the manifest the
/// CUDA builds link against.
///
/// Emitted as a bare array literal that `utils.rs` wraps, so the constant
/// keeps its documentation at the place a reader looks for it.
///
/// Every failure here is fatal on purpose. Falling back to a built-in list
/// would reintroduce exactly the second copy this file exists to remove, and
/// falling back to an empty one would reject every GPU at run time while
/// pointing at the device rather than at the unreadable manifest.
fn generate_supported_sm(solver_cpp: &str) {
    let manifest = Path::new(solver_cpp).join("cuda_arch.txt");
    println!("cargo:rerun-if-changed={}", manifest.display());

    let text = std::fs::read_to_string(&manifest).unwrap_or_else(|e| {
        panic!("cannot read the CUDA architecture manifest {}: {e}", manifest.display())
    });

    let mut cubins: Vec<u32> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let mut field = line.split_whitespace();
        let (Some("cubin"), Some(value)) = (field.next(), field.next()) else {
            continue;
        };
        let sm = value.parse::<u32>().unwrap_or_else(|e| {
            panic!("{}:{}: cubin value {value:?} is not a number: {e}", manifest.display(), n + 1)
        });
        cubins.push(sm);
    }
    if cubins.is_empty() {
        panic!("{}: no 'cubin' lines found", manifest.display());
    }

    let body = cubins.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"))
        .join("cuda_arch_cubins.rs");
    std::fs::write(&out, format!("[{body}]\n"))
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", out.display()));
}

fn main() {
    // The shared predicate header is authored in the solver's C++ tree
    // (sibling crate, source-file dependency only, not a Cargo dependency,
    // so there is no dependency cycle).
    let solver_cpp = "../ppf-cts-solver/src/cpp";
    let header = Path::new(solver_cpp).join("contact/intersect_core.hpp");

    println!("cargo:rerun-if-changed=cpp/intersect_ffi.cpp");
    println!("cargo:rerun-if-changed={}", header.display());

    generate_supported_sm(solver_cpp);

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("cpp/intersect_ffi.cpp")
        .include(solver_cpp)
        .compile("ppf_isect_ffi");
}
