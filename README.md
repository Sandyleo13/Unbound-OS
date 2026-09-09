# Unbound OS

**Your device. Your software. Your rules.**

Unbound OS is an operating system project focused on user ownership,
software freedom, security, compatibility, and deep customization.

## Goals

- User-owned computing
- Native applications
- Linux application compatibility
- Android application compatibility
- Sideloading
- User-controlled permissions
- Minimal unnecessary software
- Strong security without unnecessary restrictions
- Deep customization
- Desktop and eventually mobile support

## Philosophy

Security should protect the user, not take ownership away from the user.

## Current Development Status

Unbound OS is in early development. The current bootstrap path is a custom
BIOS-first boot chain built specifically for Unbound OS.

### Working

- x86_64 development target
- QEMU boot testing
- Custom BIOS Stage 1 bootloader
- Custom Stage 2 bootloader
- 16-bit real mode startup
- Protected mode transition
- x86_64 long mode transition
- Basic paging setup
- ELF64 kernel loading
- Custom kernel entry handoff
- Versioned `BootInfo` structure passed from the bootloader to the kernel
- Kernel-side BootInfo validation
- Serial debug output from bootloader and kernel
- Custom disk image builder without the Rust `bootloader` crate

### Current Boot Flow

```text
BIOS
  -> Unbound Stage 1
  -> Unbound Stage 2
  -> x86_64 long mode
  -> ELF64 kernel loader
  -> BootInfo handoff
  -> Unbound kernel