#!/usr/bin/env bash
# Runs the full verification, in order, and stops at the first failure:
#
#   1. cargo fmt --check
#   2. cargo clippy --workspace --all-targets -- -D warnings
#   3. cargo test --workspace             (with DIMORA_REQUIRE_GPU=1)
#   4. cargo doc --workspace --no-deps    (with RUSTDOCFLAGS="-D warnings")
#
# Works on macOS and Linux (bash, no GNU-only flags). Exit status is 0 only if every step passed;
# otherwise it is the failing command's status, and the failing step is named on stderr.
#
# DIMORA_REQUIRE_GPU=1 makes the GPU tests fail instead of skipping when no GPU adapter exists. macOS uses
# Metal and needs nothing; Linux needs a Vulkan driver (`sudo apt install mesa-vulkan-drivers`).

set -u

cd "$(dirname "$0")/.." || exit 1

step_number=0
total_steps=4

run_step() {
    description=$1
    shift
    step_number=$((step_number + 1))
    printf '\n==> [%s/%s] %s\n' "$step_number" "$total_steps" "$description"
    "$@"
    status=$?
    if [ "$status" -ne 0 ]; then
        printf '\nFAILED at step %s/%s: %s (exit status %s)\n' "$step_number" "$total_steps" "$description" "$status" >&2
        exit "$status"
    fi
}

run_step "cargo fmt --check" \
    cargo fmt --check

run_step "cargo clippy --workspace --all-targets -- -D warnings" \
    cargo clippy --workspace --all-targets -- -D warnings

run_step "cargo test --workspace (DIMORA_REQUIRE_GPU=1)" \
    env DIMORA_REQUIRE_GPU=1 cargo test --workspace

run_step "cargo doc --workspace --no-deps (RUSTDOCFLAGS=\"-D warnings\")" \
    env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

printf '\nAll %s steps passed.\n' "$total_steps"
