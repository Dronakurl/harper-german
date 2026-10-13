> **Stand:** Der Umzug (Abschnitt 2) ist gemacht. Abweichungen vom ursprünglichen Plan:
> - **Morphologie** steckt nicht in einer Seitentabelle, sondern in `harper_core::morphology`
>   hinter dem Cargo-Feature `morphology` (standardmäßig aus, Englisch bleibt unverändert). Eine
>   Seitentabelle hätte bedeutet, die Affix-Expansion von `harper-core` zu kopieren.
> - **`detect`** (Spracherkennung) ist noch nicht portiert: Der alte Detektor braucht das englische
>   Wörterbuch, das ins Modul zu legen wäre. Kommt in Phase 6.
> - **Zeitbudget** des Kompositum-Prüfers: im Modul gibt es keine Uhr (`src/clock.rs`), das Budget
>   läuft dort nie ab.
> - **Pack** wird mit `just bake-pack` erzeugt und per `include_bytes!` ins Modul gelegt; es gibt keine
>   Umgebungsvariable `HARPER_GERMAN_PACK` mehr.
> - Die Abhängigkeit zeigt auf eine feste Revision statt auf einen Tag, weil Tags in `dronakurl/harper`
>   derzeit nicht gesetzt werden können.

# Plan für dieses Repo: Deutsch als Harper-Plugin

Ergänzt den Gesamtplan
[`LANGUAGE_PLUGINS_PLAN.md`](https://github.com/Dronakurl/harper/blob/claude/language-plugin-architecture-h8bw6z/LANGUAGE_PLUGINS_PLAN.md)
(im Folgenden „Gesamtplan“). Dort stehen die Plugin-Schnittstelle (§5) und die
Phasen (§8). Hier steht nur, was **in diesem Repo** passiert.

**Voraussetzung:** Phase 0 (Spike) und Phase 1 (`harper-plugin-api`,
`harper-plugin-host`) aus dem Gesamtplan sind in `dronakurl/harper` erledigt und
mit einem Tag versehen. Vorher hier nichts bauen außer dem Spike.

## 1. Zielstruktur

```
harper-german/
├── Cargo.toml                 # crate-type = ["cdylib", "rlib"]
├── rust-toolchain.toml        # gleiche Toolchain wie dronakurl/harper, + wasm32-unknown-unknown
├── build.rs                   # backt das expandierte Wörterbuch (Pack) ins Modul
├── src/
│   ├── lib.rs                 # impl LanguagePlugin for German; export_plugin!(German)
│   ├── dialects.rs
│   ├── language_detection.rs  # → Methode `detect` (Phase 6)
│   ├── lexing.rs
│   ├── morphology.rs          # aus harper-core/src/language/morphology.rs
│   ├── grammar/
│   ├── linting/               # Rust-Linter + weir_rules/
│   ├── parsers/
│   └── spell/                 # compact_dict, compound_checker, german_dict, lexical_classes …
├── data/
│   ├── dictionary.dict
│   ├── annotations.json
│   ├── compound_begin_forms.txt
│   └── wikidata_names.tsv
├── tests/                     # Unit-/Integrationstests, batteries/*.tsv
├── test_sources/              # Beispieltexte für Präzision
├── scripts/                   # Recall-, Battery-, Treebank-, Prune-Skripte
├── justfile                   # build-wasm, test, perf, recall, battery, release
├── AGENTS.md                  # aus harper-core/src/language/AGENTS.md, angepasst
└── .github/workflows/
    ├── ci.yml                 # cargo test (nativ), wasm-Build, Ladetest mit harper-plugin-host
    └── release.yml            # harper-de.wasm + .sha256 + plugin-index.json als Release-Assets
```

Abhängigkeiten (`Cargo.toml`):

```toml
[dependencies]
harper-core       = { git = "https://github.com/Dronakurl/harper", tag = "plugins-v1", default-features = false }
harper-plugin-api = { git = "https://github.com/Dronakurl/harper", tag = "plugins-v1" }

[dev-dependencies]
harper-plugin-host = { git = "https://github.com/Dronakurl/harper", tag = "plugins-v1" }
```

Der Tag (`plugins-v1` ist ein Platzhalter) wird bewusst fest gesetzt und nur
bewusst angehoben.

## 2. Umzug des deutschen Codes

Quelle: `dronakurl/harper`, Branch `feature/german-language-support`, Ordner
`harper-core/src/language/german/` plus `harper-core/src/language/morphology.rs`.

1. **Kopieren mit Historie** (optional, aber empfehlenswert):
   `git filter-repo --path harper-core/src/language/german/ --path harper-core/src/language/morphology.rs --path-rename harper-core/src/language/german/:src/`
   auf einem frischen Klon des Feature-Branches, dann hier hineinmergen.
   Ohne Historie reicht ein einfaches Kopieren.
2. **`crate::` umschreiben:** `crate::language::german::…` → `crate::…`,
   `crate::linting::…` / `crate::spell::…` / `crate::Document` usw. →
   `harper_core::…`.
3. **Was nicht mehr gebraucht wird:** `module.rs` (`LanguageModule`-Trait und
   Registry gibt es im Plugin-Modell nicht), `config.toml` (ersetzt durch
   `PluginInfo`), `stats.rs`/`lang_stats`-Anbindung (bei Bedarf später als
   eigenes CLI in diesem Repo).
4. **Interna von harper-core**, auf die der Code zugreift: siehe Gesamtplan
   §7.1. Kurzfassung:

   | Was | Hier tun |
   |---|---|
   | `DictWordMetadata.morphology` | Seitentabelle in `CompactDict` (Wort-ID → `Morphology`); `MorphologyExt` liest dort. Kein Feld in Core |
   | `Parser::is_english` (Brill/Chunker überspringen) | Auf den Core-PR warten; bis dahin auf einen Tag des Plugin-Branches zeigen |
   | `spell::rune` (`parse_word_list`, `AttributeList`), `new_word_order`-Fix | Core-PR „öffentlich machen“ bzw. Bugfix-PR |
   | `lexing::lex_*`, `fst_dictionary`-Helfer, `word_hash` | Kleines hierher kopieren, Größeres per Core-PR öffentlich machen |
   | `Markdown/OrgMode::with_inline_parser` | Core-PR |
   | `.pack` über `HARPER_GERMAN_PACK` / Datei neben dem Binary | Entfällt: `build.rs` erzeugt das Pack, `include_bytes!` backt es ins Modul (Wasm hat kein Dateisystem). Die Expansion bleibt als Fallback und für Tests |

5. **`impl LanguagePlugin for German`** (`src/lib.rs`):
   - `info()`: `id: "de"`, `name: "Deutsch"`, Dialekte aus `dialects.rs`
     (bisher `config.toml`).
   - `new()`: baut `CompactDict` + `CompoundChecker` lazy, einmal pro Instanz.
   - `lint()`: Parser je `format` wählen (Plain/Markdown/Org mit deutschem
     Inline-Parser), `Document` bauen, kuratierte `LintGroup` laufen lassen,
     Ergebnisse in `PluginLint` umwandeln. Bei `regions` jede Region einzeln
     linten und Spans verschieben.
   - `set_user_words()`: Nutzerwörter als `MutableDictionary` vor das
     `CompactDict` mergen.

## 3. Abnahme

- `cargo test` (nativ) grün, alle bisherigen deutschen Tests übernommen.
- **Gleiche Ergebnisse wie vorher:** Auf `test_sources/` und den Batterien
  liefert `harper-de.wasm` über `harper-plugin-host` dieselben Lints wie
  `harper-cli --dialect de` vom Feature-Branch (sortiert vergleichen, siehe
  AGENTS.md „Diff sorted output“). Jede Abweichung begründen.
- Präzision/Recall-Skripte laufen gegen das Plugin (über eine kleine
  `harper-de-cli` in diesem Repo, die das Plugin lädt) statt gegen
  `harper-cli --dialect de`.
- Messwerte (nicht in README schreiben, nur im PR nennen): Modulgröße nach
  `wasm-opt -Oz`, Instanziierungszeit, Lint-Zeit vs. nativ, Spitzenspeicher.

## 4. Release

`release.yml` bei Tag `vX.Y.Z`:

1. `cargo build --release --target wasm32-unknown-unknown`
2. `wasm-opt -Oz -o harper-de.wasm …`
3. `sha256sum harper-de.wasm > harper-de.wasm.sha256`
4. Ladetest mit `harper-plugin-host` (ABI-Version, `info`, ein `lint`).
5. `plugin-index.json` erzeugen:
   `{"id":"de","version":"X.Y.Z","abi":1,"url":"…/harper-de.wasm","sha256":"…"}`
6. Alles als Release-Assets hochladen. Harper findet neue Versionen über diese
   Datei.

## 5. Später

- Begleit-Erweiterung „Harper – Deutsch“ für Chrome/Firefox (Gesamtplan §6.4),
  eigener Ordner `browser-extension/` hier oder eigenes Repo.
- Portugiesisch, Slowakisch, Polnisch: je ein eigenes Repo nach diesem Muster.
  Was sich dabei als gemeinsam herausstellt (z. B. `morphology.rs`), wandert in
  eine kleine gemeinsame Crate.
