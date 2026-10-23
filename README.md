# harper-german

Deutsche Sprachunterstützung für [Harper](https://github.com/Automattic/harper) als
**nachinstallierbares Sprachpaket** (WebAssembly-Plugin).

> **Status: benutzbar.** `harper-de.wasm` wird bei jedem Push auf `main` als Release
> [`latest`](https://github.com/Dronakurl/harper-german/releases/tag/latest) veröffentlicht, lädt in
> `harper-cli` und `harper-ls` und erkennt deutsche Texte von selbst. Offen: Browser-Erweiterung und
> Obsidian testen, Dialekte (siehe unten). Siehe [Plan und Status](https://github.com/Dronakurl/harper-german/issues/1).

## Bauen und ausprobieren

```bash
cargo install just            # einmalig
just test                     # deutsche Tests, nativ
just test-plugin              # Modul bauen, laden, damit prüfen (braucht das Ziel wasm32-unknown-unknown)
```

## In Harper installieren

Das Plugin ist **eine einzige Datei**, `harper-de.wasm`. Das Release [`latest`](https://github.com/Dronakurl/harper-german/releases/tag/latest) wird bei jedem Push auf `main`
ersetzt und enthält sie, mit Prüfsumme; direkter Link: <https://github.com/Dronakurl/harper-german/releases/download/latest/harper-de.wasm>. Im Harper-Checkout:

```bash
cargo install just                             # einmalig
just install                                   # harper-ls und harper-cli, nur Englisch (baut aus den Quellen, je nach Rechner 10–30 Minuten)
just language-install Dronakurl/harper-german  # lädt harper-de.wasm (44 MB) aus dem Release `latest` und installiert sie
harper-cli plugin list                         # de  Deutsch (harper-de.wasm)  v0.1.0  dialects: Standard, Austrian, Swiss
harper-cli lint --language de notizen.md
# harper-ls erkennt Deutsch von selbst ("language": "auto"); nur Deutsch: "harper-ls": { "language": "de" }
```

`harper-cli lint` endet mit dem Status 1 und `Error: Lints were found`, sobald es etwas meldet; das ist
kein Absturz.

Selbst gebaut: `just build-wasm`, dann `harper-cli plugin install target/wasm32-unknown-unknown/release/harper_german.wasm`.
Obsidian, Browser-Erweiterung und `harper.js` nehmen dieselbe Datei; wie, steht in Harpers Anleitung
[Other Languages](https://github.com/Dronakurl/harper/blob/claude/language-plugin-architecture-h8bw6z/packages/web/src/routes/docs/integrations/language-plugins/+page.md).

Das Modul ist etwa 45 MB groß (gepackt ca. 16 MB): fast alles davon ist das deutsche Wörterbuch.
Gemessen mit `just test-plugin` (Release-Build, ein Rechner, Zahlen als Größenordnung):
das erste Laden einer neuen Datei kompiliert das Modul, rund 6,5 s (Harper merkt sich das Ergebnis im
Cache-Ordner, danach dauert das Laden etwa 35 ms); die erste Prüfung baut das Wörterbuch, rund 0,8 s;
jede weitere Prüfung eines kurzen Textes dauert wenige Millisekunden.

## Dialekte

Das Plugin kennt Deutschland (`Standard`), Österreich (`Austrian`, `de-AT`) und die Schweiz (`Swiss`,
`de-CH`). Für die Schweiz akzeptiert die Rechtschreibprüfung *ss* statt *ß* (*Strasse*, *heisst*); sonst
prüfen alle drei gleich. Den Dialekt muss das Programm, das das Plugin lädt, in der Anfrage mitschicken
(`LintRequest::dialect`); `harper-cli` und `harper-ls` tun das noch nicht.

## Wie gut ist es?

Gemessen auf Texten, die das Projekt nicht kennt (Oktober 2026; Werkzeuge in `scripts/` und
`examples/lint_tsv.rs`):

| Korpus | Ergebnis |
|---|---|
| 30 000 Sätze Nachrichten, Web und Wikipedia (Leipzig Corpora 2021–2023) | 27 % der Sätze mit einer Meldung; fast alle davon unbekannte Namen und englische Wörter, die Grammatikregeln melden zusammen gut 600-mal |
| 4 840 korrigierte Lernersätze (Falko-MERLIN) | 7,6 % der Sätze mit einer Meldung |
| dieselben Sätze vor der Korrektur | 82 % der Rechtschreibfehler, 26 % aller annotierten Fehler gefunden |

Die meisten Fehlalarme sind Eigennamen, die das Wörterbuch nicht kennt: Sie lassen sich mit einem eigenen
Wörterbuch (`harper-ls`: „Add to dictionary“) abstellen.

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
