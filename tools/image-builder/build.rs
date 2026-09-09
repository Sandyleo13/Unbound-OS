fn main() {
    println!("cargo:rerun-if-changed=../../target/unbound-kernel.elf");
    println!("cargo:rerun-if-changed=../../boot/stage1/stage1.bin");
    println!("cargo:rerun-if-changed=../../boot/stage2/stage2.bin");
}
