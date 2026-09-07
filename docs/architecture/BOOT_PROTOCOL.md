# Unbound Boot Protocol

## Purpose

Define the interface between the Unbound bootloader and the Unbound kernel.

## Initial target

- Architecture: x86_64
- Firmware: BIOS
- Execution environment: QEMU
- Kernel format: ELF64
- Bootloader: Unbound-owned implementation

## Boot flow

BIOS
  |
  v
Unbound Stage 1
  |
  v
Unbound Stage 2
  |
  v
Unbound x86_64 loader
  |
  v
Load kernel ELF64
  |
  v
Prepare CPU execution environment
  |
  v
Transfer control to kernel entry point

## Kernel handoff

The bootloader will eventually provide the kernel with a defined boot information structure containing:

- Memory map
- Kernel physical/virtual address information
- Framebuffer information when available
- Boot command line
- Firmware/platform information

The exact binary layout will be versioned before implementation.

## Compatibility rule

The bootloader and kernel must communicate through an explicit Unbound-owned ABI.

Neither component should depend on the private implementation details of the other.

## Versioning

Boot protocol version: 0.1

This protocol is experimental and may change before the first stable Unbound release.
