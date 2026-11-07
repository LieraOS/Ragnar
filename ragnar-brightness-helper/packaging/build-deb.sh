#!/usr/bin/env bash
# Build the ragnar-brightness-helper .deb: release binary staged by
# cargo-deb per [package.metadata.deb] in Cargo.toml.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
cargo deb --no-build
