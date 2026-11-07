#!/usr/bin/env bash
# Build the ragnar-automount .deb via cargo-deb.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
cargo deb --no-build
