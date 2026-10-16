# harper-german

Deutsche Sprachunterstützung für [Harper](https://github.com/Automattic/harper) als
**nachinstallierbares Sprachpaket** (WebAssembly-Plugin).

> **Status: erster Umzug fertig.** Der deutsche Code ist hierher übernommen, läuft als
> `harper-de.wasm` und liefert über `harper-plugin-host` dieselben Lints wie nativ (siehe
> [`tests/plugin_host.rs`](tests/plugin_host.rs)). Offen: Release, Kommentare im Quellcode,
> automatische Spracherkennung, Browser. Siehe [Plan und Status](https://github.com/Dronakurl/harper-german/issues/1).

## Bauen und ausprobieren

```bash
cargo install just            # einmalig
just test                     # deutsche Tests, nativ
just test-plugin              # Modul bauen, laden, damit prüfen (braucht das Ziel wasm32-unknown-unknown)
```

Mit `harper-ls` oder `harper-cli` aus [`dronakurl/harper`](https://github.com/Dronakurl/harper/tree/claude/language-plugin-architecture-h8bw6z):

```bash
harper-cli plugin install target/wasm32-unknown-unknown/release/harper_german.wasm   # nennt sie harper-de.wasm
harper-cli lint --language de "Die Katze schlaeft."
# harper-ls: Einstellung  "harper-ls": { "language": "de" }
```

Das Modul ist etwa 45 MB groß (gepackt ca. 16 MB): fast alles davon ist das deutsche Wörterbuch.
Das erste Prüfen nach dem Laden dauert rund eine halbe Sekunde, danach wenige Millisekunden.

## Lokal zusammen mit Harper entwickeln

`Cargo.toml` hängt an einer festen Revision von `dronakurl/harper`. Um beides gleichzeitig zu ändern,
Harper daneben auschecken und in `.cargo/config.toml` (nicht einchecken) ergänzen:

```toml
[patch."https://github.com/Dronakurl/harper"]
harper-core = { path = "../harper/harper-core" }
harper-plugin-api = { path = "../harper/harper-plugin-api" }
harper-plugin-host = { path = "../harper/harper-plugin-host" }
```

## Idee in einem Satz

Harper bekommt einen „Steckplatz“ für Sprachen; dieses Repo baut das „Steckmodul“
`harper-de.wasm`, das in allen Harper-Programmen funktioniert: Browser-Erweiterung,
Website, Obsidian, harper-ls (VS Code, Neovim, Zed, …), Harper Desktop und harper-cli.

- Deutsch belegt nur Speicher, wenn gerade deutscher Text geprüft wird.
- Harper selbst (`master`) enthält keinen deutschen Code.
- Ein Paket kann automatisch heruntergeladen oder von Hand in den Plugin-Ordner gelegt werden.

## Wo steht was

| Dokument | Inhalt |
|---|---|
| [Plan und Status (Issue #14)](https://github.com/Dronakurl/harper/issues/14) in `dronakurl/harper` | Gesamtarchitektur, Plugin-Schnittstelle (ABI v1), Änderungen an Harper selbst |
| [Plan und Status (Issue #1)](https://github.com/Dronakurl/harper-german/issues/1) | Was in diesem Repo zu tun ist: Aufbau, Umzug des deutschen Codes, Build und Release |

## Lizenz

Apache-2.0, wie Harper. Der Code stammt aus einem Fork von Harper.
