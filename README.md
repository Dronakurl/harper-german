# harper-german

Deutsche Sprachunterstützung für [Harper](https://github.com/Automattic/harper) als
**nachinstallierbares Sprachpaket** (WebAssembly-Plugin).

> **Status: Planung.** Dieses Repo enthält noch keinen Code. Der deutsche Code liegt
> derzeit auf dem Branch
> [`feature/german-language-support`](https://github.com/Dronakurl/harper/tree/feature/german-language-support)
> in `dronakurl/harper` (Ordner `harper-core/src/language/german/`) und wird gemäß
> [PLAN.md](PLAN.md) hierher übernommen.

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
| [`LANGUAGE_PLUGINS_PLAN.md`](https://github.com/Dronakurl/harper/blob/claude/language-plugin-architecture-h8bw6z/LANGUAGE_PLUGINS_PLAN.md) in `dronakurl/harper` | Gesamtarchitektur, Plugin-Schnittstelle (ABI v1), Änderungen an Harper selbst |
| [PLAN.md](PLAN.md) (hier) | Was in diesem Repo zu tun ist: Aufbau, Umzug des deutschen Codes, Build und Release |

## Lizenz

Apache-2.0, wie Harper. Der Code stammt aus einem Fork von Harper.
