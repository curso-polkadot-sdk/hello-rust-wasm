#!/usr/bin/env bash
set -eu

# Check for 'rustup' and abort if it is not available.
rustup -V > /dev/null 2>&1 || { echo >&2 "ERROR - requires 'rustup' for compile the binaries"; exit 1; }

# Check if the wasm-opt is installed
if ! command -v wasm-opt &> /dev/null; then
    echo "wasm-opt is not installed. Please install it by running:"
    echo "cargo install wasm-opt"
    exit 1
fi

# Check if the wasm-tools is installed
if ! command -v wasm-tools &> /dev/null; then
    echo "wasm-tools is not installed. Please install it by running:"
    echo "cargo install wasm-tools --version 1.240.0 --locked --force"
    exit 1
fi

# Check if the awk is installed
if ! command -v awk &> /dev/null; then
    echo "awk is not installed."
    exit 1
fi

# Check if the head is installed
if ! command -v head &> /dev/null; then
    echo "head is not installed."
    exit 1
fi

# Helper method that join elements of an array
join_by(){
    local sep
    local out
    if [[ "$#" -lt 2 ]]; then
        echo "join_by: Illegal number of parameters"
        exit 1
    fi
    sep="${1}"
    out="${2}"
    shift 2
    while [[ "$#" -gt 0 ]]; do
        out="$(printf "%s${sep}%s" "${out}" "${1}")"
        shift 1
    done
    printf '%s' "${out}"
}

# Compare the semantic versioning
semver_le() {
    local RE='[^0-9]*\([0-9]*\)[.]\([0-9]*\)[.]\([0-9]*\)\([0-9A-Za-z-]*\)'
    local MAJOR_A="$(echo $1 | sed -e "s#$RE#\1#")"
    local MINOR_A="$(echo $1 | sed -e "s#$RE#\2#")"
    local PATCH_A="$(echo $1 | sed -e "s#$RE#\3#")"
    local SPECIAL_A="$(echo $1 | sed -e "s#$RE#\4#")"

    local MAJOR_B="$(echo $2 | sed -e "s#$RE#\1#")"
    local MINOR_B="$(echo $2 | sed -e "s#$RE#\2#")"
    local PATCH_B="$(echo $2 | sed -e "s#$RE#\3#")"
    local SPECIAL_B="$(echo $2 | sed -e "s#$RE#\4#")"

    if test "${MAJOR_A}" -lt "${MAJOR_B}"; then
        return 0
    elif test "${MAJOR_A}" -gt "${MAJOR_B}"; then
        return 1
    elif test "${MINOR_A}" -lt "${MINOR_B}"; then
        return 0
    elif test "${MINOR_A}" -gt "${MINOR_B}"; then
        return 1
    elif test "${PATCH_A}" -lt "${PATCH_B}"; then
        return 0
    elif test "${PATCH_A}" -gt "${PATCH_B}"; then
        return 1
    fi
    return 1
}

# Make sure we are in workspace root directory
cd "$(dirname "${0}")"

# For Rust >= 1.70 and Rust < 1.84 with `wasm32-unknown-unknown` target,
# it's required to disable default WASM features:
# - `sign-ext` (since Rust 1.70)
# - `multivalue` and `reference-types` (since Rust 1.82)
#
# For Rust >= 1.84, we use `wasm32v1-none` target
# (disables all "post-MVP" WASM features except `mutable-globals`):
# - https://doc.rust-lang.org/beta/rustc/platform-support/wasm32v1-none.html
# 
# Also see:
# https://blog.rust-lang.org/2024/09/24/webassembly-targets-change-in-default-target-features.html#disabling-on-by-default-webassembly-proposals
rustc_version="$(cargo --version | head -n1 | awk '{ print $2 }')"

if semver_le '1.84.0' "${rustc_version}"; then
    RUST_TARGET='wasm32v1-none'
    rust_toolchain="${rustc_version}"
else
    RUST_TARGET='wasm32-unknown-unknown'
    rust_toolchain="nightly-2025-11-09"
fi
echo "rust version: ${rustc_version}"
echo "      target: ${RUST_TARGET}"

# Check if the target `wasm32v1-none` or `wasm32-unknown-unknown` is installed
if ! rustup target list | grep -q "${RUST_TARGET}"; then
  echo "Installing the target with rustup '${RUST_TARGET}'"
  rustup target add "${RUST_TARGET}" --toolchain "${rust_toolchain}"
fi

####################################################################
# STEP 1: Build the project with the wasm32-unknown-unknown target #
####################################################################
# Set the stack size to 64KB
STACK_SIZE=65536

# Wasm Features supported by rustc, run the command below to list them:
# rustc -Ctarget-feature=help --target wasm32-unknown-unknown
RUST_WASM_FEATURES=(
    # enabled
    +mutable-globals
    # disabled
    -atomics
    -bulk-memory
    -crt-static
    -exception-handling
    -extended-const
    -multivalue
    -nontrapping-fptoint
    -reference-types
    -relaxed-simd
    -sign-ext
    -simd128
    -tail-call
    -wide-arithmetic
    -bulk-memory-opt
    -call-indirect-overlong
    -fp16
    -multimemory
)
RUST_WASM_FEATURES="$(join_by ',' "${RUST_WASM_FEATURES[@]}")"

# List of custom flags to pass to all compiler invocations that Cargo performs.
RUST_WASM_FLAGS=(
    # Max wasm stack size
    "-Clink-arg=-zstack-size=${STACK_SIZE}"
    # Configure the wasm target to import instead of export memory
    '-Clink-arg=--import-memory'
    # Max wasm stack size
    "-Ctarget-feature=${RUST_WASM_FEATURES}"
    # Defers the LTO optimization to the actual linking step
    # '-Clinker-plugin-lto'
    # Export __indirect_function_table
    # '-Clink-arg=--export-table'
    # Deny warnings
    '-Dwarnings'
)

rustc_extra_args=()
if [ "${RUST_TARGET}" = "wasm32-unknown-unknown" ]; then
    # List of custom flags to pass to all compiler invocations that Cargo performs.
    RUST_WASM_FLAGS+=('-Ctarget-cpu=mvp')
    rustc_extra_args+=("+${rust_toolchain}")
    if [[ "${rust_toolchain}" == nightly* ]]; then
        rustc_extra_args+=('-Zbuild-std=core,alloc')
    fi
fi

# Separated flags by 0x1f (ASCII Unit Separator)
# Reference: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-reads
RUST_WASM_FLAGS="$(join_by "\x1f" "${RUST_WASM_FLAGS[@]}")"

RUST_BUILD_VARS=(
    "set CARGO_ENCODED_RUSTFLAGS=${RUST_WASM_FLAGS@Q};"
    "set SOURCE_DATE_EPOCH='1600000000';"
    "set TZ='UTC';"
    "set LC_ALL='C';"
)

RUST_BUILD_CMD=(
    'cargo'
    "${rustc_extra_args[@]}"
    'build'
    '--package=wasm-runtime'
    '--profile=release'
    "--target=${RUST_TARGET}"
    '--no-default-features'
)

# Build the project using `wasm32v1-none` or `wasm32-unknown-unknown`` target
printf '     COMMAND:\n%s\\\n%s\n\n' \
    "$(join_by '\\\n' "${RUST_BUILD_VARS[@]}")" \
    "$(join_by '\x20' "${RUST_BUILD_CMD[@]}")"

CARGO_ENCODED_RUSTFLAGS="${RUST_WASM_FLAGS}" \
    cargo \
    "${rustc_extra_args[@]}" \
    build \
    --package=wasm-runtime \
    --profile=release \
    --target="${RUST_TARGET}" \
    --no-default-features

wasm-tools \
    print \
    ./"target/${RUST_TARGET}/release/wasm_runtime.wasm" > ./wasm_runtime.wat

###############################################################
# step 2 - Remove unnecessary code and optimize the wasm file #
###############################################################
# Run `wasm-opt --help` to see all available options
WASM_OPT_OPTIONS=(
    -O4
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
    --strip-dwarf
    --signext-lowering
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
    --enable-mutable-globals
    --disable-simd
    --disable-threads
    --disable-gc
    --disable-multivalue
    --disable-reference-types
    --disable-exception-handling
    --disable-fp16
    --disable-sign-ext
    --disable-multimemory
    --disable-bulk-memory
    --disable-bulk-memory-opt
    --optimize-stack-ir
    --vacuum
    # --unsubtyping
)

# Remove existing `wasm_runtime.wasm` and `wasm_runtime.wat` files
rm ./wasm_runtime.wasm ./wasm_runtime.wat 2> /dev/null || true

# Create the `wasm_runtime.wasm` file (binary format)
wasm-opt \
    "${WASM_OPT_OPTIONS[@]}" \
    --output ./wasm_runtime.wasm \
    ./"target/${RUST_TARGET}/release/wasm_runtime.wasm"

#######################################################
# step 3 - Convert from binary (.wasm) to text (.wat) #
#######################################################
# Create the `wasm_runtime.wat` file (text format)
wasm-tools strip ./wasm_runtime.wasm -o ./wasm_runtime.wasm
wasm-tools print ./wasm_runtime.wasm > ./wasm_runtime.wat
# wasm-opt --print-function-map ./wasm_runtime.wasm

wc -c ./wasm_runtime.wasm
md5sum ./wasm_runtime.wasm

# Print the `wasm_runtime.wat` file
printf "\nSuccess !!!\n"
