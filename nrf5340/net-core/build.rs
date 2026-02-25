use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use memory_spec::MemorySpec;

fn memory_x() -> String {
    let content = include_str!("../memory.kdl");
    let memoryspec = MemorySpec::from_str(&content).unwrap();

    let flash = &memoryspec.regions()["netcore_flash"];
    let flash_origin = flash.origin();
    let flash_length = flash.length();
    let ram = &memoryspec.regions()["netcore_ram"];
    let ram_origin = ram.origin();
    let ram_length = ram.length();
    let symbols = memoryspec.render_symbols();
    format!(
        "\
MEMORY
{{
  FLASH : ORIGIN = 0x{flash_origin:08x}, LENGTH = 0x{flash_length:04x}
  RAM : ORIGIN = 0x{ram_origin:08x}, LENGTH = 0x{ram_length:04x}
}}
{symbols}"
    )
}

fn main() {
    let memory_x_content = memory_x();

    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(memory_x_content.as_bytes())
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.kdl`
    // here, we ensure the build script is only re-run when
    // `memory.kdl` is changed.
    println!("cargo:rerun-if-changed=../memory.kdl");

    // This is needed if your flash or ram addresses are not aligned to 0x10000 in memory.x
    // See https://github.com/rust-embedded/cortex-m-quickstart/pull/95
    println!("cargo:rustc-link-arg-bins=--nmagic");

    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=--print-memory-usage");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
