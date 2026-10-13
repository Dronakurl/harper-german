default:
  @just --list

wasm := "target/wasm32-unknown-unknown/release/harper_german.wasm"

# Format all Rust code.
format:
  cargo fmt --all

# What CI checks first.
check:
  cargo fmt --all -- --check
  cargo clippy --all-targets -- -D warnings

# The native tests (the German linters, dictionary and grammar).
test:
  cargo test --lib

# Expand the word list into data/german.pack, which build.rs bakes in (else it is expanded at first use).
bake-pack:
  cargo run --release --example bake_pack

# Build the plugin. Needs `rustup target add wasm32-unknown-unknown`.
build-wasm: bake-pack
  cargo build --release --target wasm32-unknown-unknown
  @ls -l {{wasm}}

# Load the built module through harper-plugin-host and lint with it.
test-plugin: build-wasm
  cargo test --release --test plugin_host -- --nocapture --test-threads=1

# Produce dist/harper-de.wasm, its checksum and plugin-index.json. VERSION like 0.1.0.
release-files VERSION: build-wasm
  #!/usr/bin/env bash
  set -euo pipefail
  mkdir -p dist
  if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-bulk-memory --enable-sign-ext --enable-nontrapping-float-to-int \
      -o dist/harper-de.wasm {{wasm}}
  else
    echo "wasm-opt not found; shipping the unoptimized module" >&2
    cp {{wasm}} dist/harper-de.wasm
  fi
  (cd dist && sha256sum harper-de.wasm > harper-de.wasm.sha256)
  sha=$(cut -d' ' -f1 dist/harper-de.wasm.sha256)
  cat > dist/plugin-index.json <<JSON
  {"id":"de","version":"{{VERSION}}","abi":1,"url":"https://github.com/Dronakurl/harper-german/releases/download/v{{VERSION}}/harper-de.wasm","sha256":"$sha"}
  JSON
  ls -l dist
