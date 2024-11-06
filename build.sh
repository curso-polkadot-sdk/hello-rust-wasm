#!/bin/bash
set -e

# Check for 'rustup' and abort if it is not available.
rustup -V > /dev/null 2>&1 || { echo >&2 "ERROR - requires 'rustup' for compile the binaries"; exit 1; }

# Check if the rust target is installed
if ! rustup target list | grep -q wasm32-unknown-unknown; then
  echo "Installing the musl target with rustup 'wasm32-unknown-unknownt'"
  rustup target add wasm32-unknown-unknown
fi

# Check if the wasm-opt is installed
if ! command -v wasm-opt &> /dev/null; then
    echo "wasm-opt is not installed. Please install it by running:"
    echo "cargo install wasm-opt"
    exit 1
fi

####################################################################
# STEP 1: Build the project with the wasm32-unknown-unknown target #
####################################################################

# Set the stack size to 64KB
STACK_SIZE=65536

# List of custom flags to pass to all compiler invocations that Cargo performs.
RUST_WASM_FLAGS=(
    # Max wasm stack size
    "-Clink-arg=-zstack-size=$STACK_SIZE"
    # Configure the wasm target to import instead of export memory
    '-Clink-arg=--import-memory'
    # Doesn't optimize this build for any specific CPU
    '-Ctarget-cpu=mvp'
)

# Separated flags by 0x1f (ASCII Unit Separator)
# Reference: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-reads
RUST_WASM_FLAGS="$(printf '%s\x1f%s\x1f%s' ${RUST_WASM_FLAGS[@]})"

# Build the project with the wasm32-unknown-unknown target
printf "%s\n" "CARGO_ENCODED_RUSTFLAGS=\"$RUST_WASM_FLAGS\" cargo build -p wasm-runtime --release --target=wasm32-unknown-unknown"
CARGO_ENCODED_RUSTFLAGS="$RUST_WASM_FLAGS" cargo build -p wasm-runtime --release --target=wasm32-unknown-unknown

###############################################################
# step 2 - Remove unnecessary code and optimize the wasm file #
###############################################################
# Run `wasm-opt --help` to see all available options
WASM_OPT_OPTIONS=(
    -O3
    --dce
    --precompute
    --precompute-propagate
    --optimize-instructions
    --optimize-casts
    --low-memory-unused
    --optimize-added-constants
    --optimize-added-constants-propagate
    --simplify-globals-optimizing
    --inlining-optimizing
    --once-reduction
    --merge-locals
    --merge-similar-functions
    --strip
    --strip-debug
    # --remove-memory
    --remove-unused-names
    --remove-unused-types
    --remove-unused-module-elements
    --duplicate-function-elimination
    --duplicate-import-elimination
    --reorder-functions
    --abstract-type-refining
    --alignment-lowering
    --avoid-reinterprets
    # --zero-filled-memory
    --disable-simd
    --disable-threads
    --disable-gc
    --disable-multivalue
    --disable-reference-types
    --disable-exception-handling
    --optimize-stack-ir
    --vacuum
    # --unsubtyping
)

# Create the `wasm_runtime.wasm` file (binary format)
wasm-opt \
    "${WASM_OPT_OPTIONS[@]}" \
    --output ./wasm_runtime.wasm \
    ./target/wasm32-unknown-unknown/release/wasm_runtime.wasm

#######################################################
# step 3 - Convert from binary (.wasm) to text (.wat) #
#######################################################
# Create the `wasm_runtime.wat` file (text format)
wasm-opt \
    "${WASM_OPT_OPTIONS[@]}" \
    --emit-text \
    --output ./wasm_runtime.wat \
    ./target/wasm32-unknown-unknown/release/wasm_runtime.wasm

# Print the `wasm_runtime.wat` file
printf "\n%s\n" '--------------- WAT CODE ---------------'
cat ./wasm_runtime.wat
printf "\n%s\n" '----------------------------------------'
printf "\nSuccess !!!\n"