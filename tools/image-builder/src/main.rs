use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

const SECTOR_SIZE: usize = 512;
const STAGE1_SECTORS: usize = 1;
const STAGE2_SECTORS: usize = 32;
const KERNEL_LBA: usize = STAGE1_SECTORS + STAGE2_SECTORS;

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.join("../..");

    let stage1_path = root.join("boot/stage1/stage1.bin");
    let stage2_path = root.join("boot/stage2/stage2.bin");
    let kernel_path = root.join("target/unbound-kernel-stripped.elf");

    let output_dir = root.join("target");
    fs::create_dir_all(&output_dir)?;

    let image_path = output_dir.join("unbound-os.img");

    let stage1 = fs::read(&stage1_path)?;
    let stage2 = fs::read(&stage2_path)?;
    let kernel = fs::read(&kernel_path)?;

    if stage1.len() != SECTOR_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Stage 1 must be exactly 512 bytes, got {}",
                stage1.len()
            ),
        ));
    }

    if stage2.len() > STAGE2_SECTORS * SECTOR_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Stage 2 is too large: {} bytes, maximum is {} bytes",
                stage2.len(),
                STAGE2_SECTORS * SECTOR_SIZE
            ),
        ));
    }

    if kernel.len() < 4 || &kernel[0..4] != b"\x7FELF" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Kernel is not a valid ELF file: {}",
                kernel_path.display()
            ),
        ));
    }

    if stage1[510] != 0x55 || stage1[511] != 0xAA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Stage 1 is missing the 0x55AA boot signature",
        ));
    }

    let kernel_sectors = (kernel.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;

    let image_size = (KERNEL_LBA + kernel_sectors) * SECTOR_SIZE;

    let mut image = vec![0u8; image_size];

    // LBA 0: Stage 1.
    image[0..SECTOR_SIZE].copy_from_slice(&stage1);

    // LBA 1..32: Stage 2.
    let stage2_offset = STAGE1_SECTORS * SECTOR_SIZE;
    let stage2_end = stage2_offset + stage2.len();
    image[stage2_offset..stage2_end].copy_from_slice(&stage2);

    // LBA 33+: kernel ELF.
    let kernel_offset = KERNEL_LBA * SECTOR_SIZE;
    let kernel_end = kernel_offset + kernel.len();
    image[kernel_offset..kernel_end].copy_from_slice(&kernel);

    let mut file = fs::File::create(&image_path)?;
    file.write_all(&image)?;
    file.flush()?;

    println!("Unbound OS image created:");
    println!("  Stage 1:        {} bytes", stage1.len());
    println!("  Stage 2:        {} bytes", stage2.len());
    println!("  Kernel ELF:     {} bytes", kernel.len());
    println!("  Kernel sectors: {}", kernel_sectors);
    println!("  Kernel LBA:     {}", KERNEL_LBA);
    println!("  Image:          {}", image_path.display());

    Ok(())
}
