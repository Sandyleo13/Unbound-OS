use std::path::Path;

fn main() {
    let kernel = std::env::var_os("CARGO_BIN_FILE_UNBOUND_KERNEL_unbound-kernel")
        .expect("kernel artifact path not found");

    let output_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap()
        .join("../../target");

    std::fs::create_dir_all(&output_dir).expect("failed to create target directory");

    let image_path = output_dir.join("unbound-os.img");

    bootloader::BiosBoot::new(Path::new(&kernel))
        .create_disk_image(&image_path)
        .expect("failed to create BIOS disk image");

    println!("cargo:rerun-if-changed=../../kernel/src");
}
