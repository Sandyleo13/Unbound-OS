use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

// ============================================================
// UNBOUND OS IMAGE BUILDER
// ============================================================
//
// Disk layout:
//
//   LBA 0        Stage 1
//   LBA 1-32     Stage 2
//   LBA 33+      Kernel ELF area
//
// Stage 2 reads a fixed-size kernel area.
//
// Current kernel capacity:
//
//   512 sectors
//   512 × 512 bytes
//   = 262144 bytes
//   = 256 KiB
//
// Kernel temporary buffer:
//
//   0x20000 - 0x60000
//
// E820 memory map:
//
//   0x60000
//
// Therefore the maximum kernel buffer ends exactly where the
// E820 memory-map buffer begins.
//
// ============================================================

// ============================================================
// DISK CONSTANTS
// ============================================================

const SECTOR_SIZE: usize = 512;

const STAGE1_SECTORS: usize = 1;

const STAGE2_SECTORS: usize = 32;

// Kernel starts immediately after Stage 1 + Stage 2.
//
// LBA:
//
//   1 + 32
//
// = 33
const KERNEL_LBA: usize = STAGE1_SECTORS + STAGE2_SECTORS;

// ============================================================
// STAGE 2 KERNEL CAPACITY
// ============================================================
//
// Stage 2 BIOS DAP:
//
//     dw 512
//
// Therefore Stage 2 can read at most 512 sectors.
//
// ============================================================

const STAGE2_KERNEL_READ_SECTORS: usize = 1024;

// ============================================================
// FILE PATHS
// ============================================================

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn stage1_path() -> PathBuf {
    project_root().join("boot/stage1/stage1.bin")
}

fn stage2_path() -> PathBuf {
    project_root().join("boot/stage2/stage2.bin")
}

fn kernel_path() -> PathBuf {
    project_root().join("target/unbound-kernel-stripped.elf")
}

fn output_path() -> PathBuf {
    project_root().join("target/unbound-os.img")
}

// ============================================================
// READ FILE
// ============================================================

fn read_file(path: &PathBuf) -> io::Result<Vec<u8>> {
    fs::read(path)
}

// ============================================================
// MAIN
// ============================================================

fn main() -> io::Result<()> {
    println!("========================================");
    println!("        UNBOUND OS IMAGE BUILDER");
    println!("========================================");
    println!();

    // ========================================================
    // Resolve paths
    // ========================================================

    let stage1_path = stage1_path();

    let stage2_path = stage2_path();

    let kernel_path = kernel_path();

    let output_path = output_path();

    // ========================================================
    // Read Stage 1
    // ========================================================

    let stage1 = read_file(&stage1_path)?;

    println!("Stage 1: {} bytes", stage1.len());

    // ========================================================
    // Validate Stage 1
    // ========================================================
    //
    // Stage 1 occupies exactly one sector.
    //
    // Required:
    //
    //     512 bytes
    //
    // BIOS boot signature:
    //
    //     0x55AA
    //
    // ========================================================

    if stage1.len() != STAGE1_SECTORS * SECTOR_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Stage 1 must be exactly {} bytes, got {}",
                STAGE1_SECTORS * SECTOR_SIZE,
                stage1.len()
            ),
        ));
    }

    if stage1[510] != 0x55 || stage1[511] != 0xAA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Stage 1 is missing BIOS boot signature 0x55AA",
        ));
    }

    // ========================================================
    // Read Stage 2
    // ========================================================

    let stage2 = read_file(&stage2_path)?;

    println!("Stage 2: {} bytes", stage2.len());

    // ========================================================
    // Validate Stage 2
    // ========================================================
    //
    // Stage 2 occupies LBA 1-32:
    //
    //     32 × 512
    //     = 16384 bytes
    //
    // Stage 2 must therefore fit exactly inside this area.
    //
    // ========================================================

    let stage2_capacity = STAGE2_SECTORS * SECTOR_SIZE;

    if stage2.len() > stage2_capacity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Stage 2 is too large: {} bytes, maximum {} bytes",
                stage2.len(),
                stage2_capacity
            ),
        ));
    }

    // ========================================================
    // Read Kernel
    // ========================================================

    let kernel = read_file(&kernel_path)?;

    println!("Kernel ELF: {} bytes", kernel.len());

    // ========================================================
    // Calculate actual kernel sectors
    // ========================================================

    let actual_kernel_sectors = (kernel.len() + SECTOR_SIZE - 1) / SECTOR_SIZE;

    if actual_kernel_sectors == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Kernel ELF is empty",
        ));
    }

    // ========================================================
    // Check Stage 2 capacity
    // ========================================================

    if actual_kernel_sectors > STAGE2_KERNEL_READ_SECTORS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Kernel is too large for current Stage 2: {} sectors, maximum {} sectors",
                actual_kernel_sectors, STAGE2_KERNEL_READ_SECTORS
            ),
        ));
    }

    // ========================================================
    // Kernel area
    // ========================================================
    //
    // Stage 2 reads the entire fixed kernel area.
    //
    // Current:
    //
    //     512 sectors
    //
    // =   262144 bytes
    //
    // The image reserves this complete area even if the kernel
    // itself is smaller.
    //
    // ========================================================

    let image_kernel_sectors = STAGE2_KERNEL_READ_SECTORS;

    let image_kernel_bytes = image_kernel_sectors * SECTOR_SIZE;

    let image_size = (KERNEL_LBA + image_kernel_sectors) * SECTOR_SIZE;

    // ========================================================
    // Print layout
    // ========================================================

    println!();
    println!("Disk layout:");
    println!("  Stage 1 LBA:       {}", 0);

    println!("  Stage 2 LBA:       1-{}", STAGE2_SECTORS);

    println!("  Kernel LBA:        {}", KERNEL_LBA);

    println!("  Kernel sectors:    {}", image_kernel_sectors);

    println!("  Kernel capacity:   {} bytes", image_kernel_bytes);

    println!("  Image size:        {} bytes", image_size);

    println!();

    // ========================================================
    // Create image
    // ========================================================

    let mut image = vec![0u8; image_size];

    // ========================================================
    // Write Stage 1
    // ========================================================

    let stage1_offset = STAGE1_SECTORS.saturating_sub(1) * SECTOR_SIZE;

    image[stage1_offset..stage1_offset + stage1.len()].copy_from_slice(&stage1);

    // ========================================================
    // Write Stage 2
    // ========================================================

    let stage2_offset = STAGE1_SECTORS * SECTOR_SIZE;

    image[stage2_offset..stage2_offset + stage2.len()].copy_from_slice(&stage2);

    // ========================================================
    // Write Kernel
    // ========================================================

    let kernel_offset = KERNEL_LBA * SECTOR_SIZE;

    image[kernel_offset..kernel_offset + kernel.len()].copy_from_slice(&kernel);

    // ========================================================
    // Flush image to disk
    // ========================================================

    let mut file = fs::File::create(&output_path)?;

    file.write_all(&image)?;

    file.flush()?;

    // ========================================================
    // Final information
    // ========================================================

    println!("Actual kernel sectors: {}", actual_kernel_sectors);

    println!("Stage 2 read sectors:  {}", STAGE2_KERNEL_READ_SECTORS);

    println!(
        "Kernel capacity:       {} sectors",
        STAGE2_KERNEL_READ_SECTORS
    );

    println!(
        "Kernel unused space:   {} bytes",
        image_kernel_bytes.saturating_sub(kernel.len())
    );

    println!();

    println!("✓ Image created:");

    println!("  {}", output_path.display());

    println!("  {} bytes", image.len());

    println!();

    Ok(())
}
