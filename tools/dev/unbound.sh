#!/usr/bin/env bash

set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

LOG_DIR="$ROOT/target/logs"
LOG_FILE="$LOG_DIR/boot-$(date +%Y%m%d-%H%M%S).log"
IMAGE="$ROOT/target/unbound-os.img"

QEMU_TIMEOUT="${UNBOUND_QEMU_TIMEOUT:-10}"

mkdir -p "$LOG_DIR"

echo "========================================"
echo "          UNBOUND OS DEV TEST"
echo "========================================"
echo
echo "Root: $ROOT"
echo "Log:  $LOG_FILE"
echo

# ------------------------------------------------------------
# 1. Build kernel
# ------------------------------------------------------------

echo "[1/4] Building kernel..."
echo

if ! cargo build -p unbound-kernel; then
    echo
    echo "ERROR: Kernel build failed."
    exit 1
fi

echo
echo "✓ Kernel build successful"
echo

# ------------------------------------------------------------
# 2. Build image
# ------------------------------------------------------------

echo "[2/4] Building disk image..."
echo

if ! cargo run -p image-builder --target x86_64-unknown-linux-gnu; then
    echo
    echo "ERROR: Image builder failed."
    exit 1
fi

if [[ ! -f "$IMAGE" ]]; then
    echo
    echo "ERROR: Image not found:"
    echo "$IMAGE"
    exit 1
fi

echo
echo "✓ Disk image created"
echo "  $IMAGE"
echo

# ------------------------------------------------------------
# 3. QEMU
# ------------------------------------------------------------

echo "[3/4] Starting QEMU..."
echo

QEMU_DEBUG_LOG="$LOG_DIR/qemu-$(date +%Y%m%d-%H%M%S).log"

echo "========================================" > "$LOG_FILE"
echo " UNBOUND OS SERIAL OUTPUT" >> "$LOG_FILE"
echo "========================================" >> "$LOG_FILE"
echo >> "$LOG_FILE"

echo "[4/4] Monitoring boot..."
echo
echo "Timeout: ${QEMU_TIMEOUT}s"
echo "Serial log: $LOG_FILE"
echo "QEMU debug log: $QEMU_DEBUG_LOG"
echo

# ------------------------------------------------------------
# Run QEMU
#
# -no-reboot prevents QEMU from automatically restarting after
# a triple fault.
#
# -no-shutdown prevents QEMU from immediately exiting on shutdown.
#
# -d int,cpu_reset enables CPU reset and interrupt/exception
# diagnostics.
# ------------------------------------------------------------

set +e

timeout --foreground "$QEMU_TIMEOUT" \
    qemu-system-x86_64 \
    -drive "format=raw,file=$IMAGE" \
    -display none \
    -serial stdio \
    -no-reboot \
    -no-shutdown \
    -d int,cpu_reset \
    -D "$QEMU_DEBUG_LOG" \
    2>&1 | tee -a "$LOG_FILE"

QEMU_STATUS="${PIPESTATUS[0]}"

set -e

echo
echo "QEMU debug log:"
echo "$QEMU_DEBUG_LOG"

if [[ -f "$QEMU_DEBUG_LOG" ]]; then
    echo
    echo "Last QEMU diagnostic lines:"
    echo "----------------------------------------"
    tail -n 50 "$QEMU_DEBUG_LOG"
    echo "----------------------------------------"
fi
QEMU_STATUS="${PIPESTATUS[0]}"

set -e

echo
echo "========================================"
echo "          BOOT TEST COMPLETE"
echo "========================================"
echo

# ------------------------------------------------------------
# Boot analysis
# ------------------------------------------------------------

BOOT_COUNT="$(grep -c "UNBOUND BOOTLOADER" "$LOG_FILE" 2>/dev/null || true)"

echo "Boot cycles detected: $BOOT_COUNT"
echo

if grep -q "KERNEL ENTRY" "$LOG_FILE"; then
    echo "✓ Kernel entry reached"
else
    echo "✗ Kernel entry not reached"
fi

if grep -q "BOOTINFO OK" "$LOG_FILE"; then
    echo "✓ BootInfo validated"
else
    echo "✗ BootInfo validation not reached"
fi

if grep -q "GDT: INIT COMPLETE" "$LOG_FILE"; then
    echo "✓ GDT initialization completed"
elif grep -q "GDT INITIALIZATION" "$LOG_FILE"; then
    echo "✗ GDT initialization did not complete"
else
    echo "✗ GDT initialization not reached"
fi

if grep -q "GDT: LGDT COMPLETE" "$LOG_FILE"; then
    echo "✓ LGDT completed"
fi

if grep -q "GDT: RELOAD SEGMENTS COMPLETE" "$LOG_FILE"; then
    echo "✓ Segment reload completed"
fi

if grep -q "GDT: LOAD TR COMPLETE" "$LOG_FILE"; then
    echo "✓ Task register loaded"
fi

if grep -q "IDT INITIALIZATION" "$LOG_FILE"; then
    echo "✓ IDT initialization reached"
fi

if grep -q "MEMORY MAP" "$LOG_FILE"; then
    echo "✓ Memory map reached"
fi

# ------------------------------------------------------------
# Detect reboot loop
# ------------------------------------------------------------

if [[ "$BOOT_COUNT" -gt 1 ]]; then
    echo
    echo "⚠ BOOT LOOP DETECTED"
    echo
    echo "Last 40 lines:"
    echo "----------------------------------------"
    tail -n 40 "$LOG_FILE"
    echo "----------------------------------------"
fi

# ------------------------------------------------------------
# Detect timeout
# ------------------------------------------------------------

if [[ "$QEMU_STATUS" -eq 124 ]]; then
    echo
    echo "✓ QEMU stopped after ${QEMU_TIMEOUT}s timeout"
fi

echo
echo "QEMU status: $QEMU_STATUS"
echo
echo "Full log:"
echo "$LOG_FILE"
echo