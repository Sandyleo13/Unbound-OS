use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../kernel/src");
    println!("cargo:rerun-if-changed=../../kernel/Cargo.toml");
    println!("cargo:rerun-if-changed=../../kernel/linker.ld");
    println!("cargo:rerun-if-changed=../../boot/stage1/stage1.bin");
    println!("cargo:rerun-if-changed=../../boot/stage2/stage2.bin");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join("../..");

    let kernel = root.join("target/x86_64-unknown-none/debug/unbound-kernel");
    let stripped = root.join("target/unbound-kernel-stripped.elf");

    if !kernel.exists() {
        panic!(
            "kernel ELF not found: {}\nBuild it first with: cargo build -p unbound-kernel",
            kernel.display()
        );
    }

    let sysroot = Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("failed to execute rustc");

    if !sysroot.status.success() {
        panic!("rustc --print sysroot failed");
    }

    let sysroot = String::from_utf8(sysroot.stdout)
        .expect("rustc returned invalid UTF-8")
        .trim()
        .to_owned();

    let host = env::var("HOST")
        .expect("Cargo HOST environment variable is not set");

    let objcopy = PathBuf::from(sysroot)
        .join("lib")
        .join("rustlib")
        .join(&host)
        .join("bin")
        .join("llvm-objcopy");

    if !objcopy.exists() {
        panic!(
            "llvm-objcopy not found at: {}",
            objcopy.display()
        );
    }

    let status = Command::new(&objcopy)
        .args(["--strip-all"])
        .arg(&kernel)
        .arg(&stripped)
        .status()
        .expect("failed to execute llvm-objcopy");

    if !status.success() {
        panic!("llvm-objcopy failed while stripping kernel ELF");
    }

    println!(
        "cargo:warning=Kernel stripped: {}",
        stripped.display()
    );
}
