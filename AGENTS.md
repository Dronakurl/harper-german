# Agent guide: harper-german

German for [Harper](https://github.com/Dronakurl/harper), built as a WebAssembly **plugin**: one
file, `harper-de.wasm`, that `harper-ls`, `harper-cli` and (later) `harper.js` load at run time.
Read `README.md` first, then the plan in https://github.com/Dronakurl/harper-german/issues/1. The plugin interface is described in
Issue https://github.com/Dronakurl/harper/issues/14.

## Layout

- `src/plugin.rs` – `impl LanguagePlugin for German` and `export_plugin!`. The only file that knows
  about the plugin interface; everything else is ordinary Rust on top of `harper-core`.
- `src/{spell,grammar,linting,parsers,lexing,dialects}` – the German code, moved here unchanged
  from `harper-core/src/language/german/`.
- `src/linting/weir_rules/de/*.weir` – Weir rules; `build.rs` turns each file into a rule and a test.
- `data/` – `dictionary.dict`, `annotations.json`, `compound_begin_forms.txt`, `wikidata_names.tsv`.
  `data/german.pack` (git-ignored, 30 MB) is the dictionary already expanded; `just bake-pack` writes it
  and `build.rs` embeds it in the module.
- `src/tests/` – unit tests (they use crate internals) and `batteries/*.tsv`.
- `tests/plugin_host.rs` – loads the built `.wasm` through `harper-plugin-host`; includes the
  wasm-equals-native check on `test_sources/`.
- `docs/` – historical notes from when German lived inside `harper-core`.

## Commands

```bash
just check          # fmt + clippy -D warnings
just test           # native unit tests (~10 s)
just build-wasm     # bake the pack, build the module
just test-plugin    # build-wasm, then load it through the host and lint with it
just release-files 0.1.0
```

## Rules

- **Do not change `harper-core` from here.** It is a dependency, pinned by `rev` in `Cargo.toml`.
  If German needs something from core, make a small, general change in Dronakurl/harper first, then
  raise the `rev` here (and bump `HARPER_CORE_VERSION` in `src/plugin.rs` if its version changed).
- **The pack must match the build.** `pack_stamp()` hashes the word list, annotations, expansion code
  and `Cargo.toml`/`Cargo.lock`. A stale pack is ignored without a warning and the module then expands
  the dictionary on first use (about a minute). After changing any of those, run `just bake-pack`.
- **No imports in the module.** There is no clock, file system or randomness inside the plugin.
  `src/clock.rs` and `src/wasm_support.rs` are the stand-ins; do not use `std::time::Instant` or
  `web-time` directly, and do not add dependencies that need `wasm-bindgen`.
- Keep `just check` and `just test-plugin` green. Compare lints before and after a change on
  `test_sources/` and the batteries; sort the output before diffing.
- Human-facing guidance that proves stable belongs in `README.md`; remind the maintainer to move it
  from this file when you notice it has settled.
