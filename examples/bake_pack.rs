//! Writes the expanded dictionary to `data/german.pack`, which `build.rs` then bakes into the
//! plugin. Run it via `just bake-pack`.

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/german.pack".to_owned());
    let pack = harper_german::spell::german_dict::bake_german_pack();
    std::fs::write(&out, &pack).unwrap();
    eprintln!("wrote {out}: {} bytes", pack.len());
}
