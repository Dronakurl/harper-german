#!/usr/bin/env python3
"""Score Harper on a per-class battery of German sentences.

A battery is a TSV of `class<TAB>err|ok<TAB>sentence`. Every `err` row is a
sentence that should be reported and every `ok` row one that should not, so
detection and false alarms are scored together — a class scored without its
correct sentences is not scored.

    just build-wasm   # and build harper-cli in the harper checkout: cargo build --release -p harper-cli
    python3 scripts/german_battery.py \
        src/tests/batteries/grammar.tsv

Three things this gets right that a quick rewrite does not:

* **Harper is batched.** One file per sentence, one invocation. Loading the
  German dictionary costs about 1.5 s and the linting costs nothing, so a
  sentence-per-process loop takes minutes instead of seconds.
* **`--format compact` prints basenames**, not the paths it was given, so
  sentences are recovered from `NNN.md` rather than from the full path.
* **Typography is excluded by rule name, never by lint kind.** `Spaces` and
  `CommaFixes` would otherwise count as a hit on the right sentence for the
  wrong reason — but `GermanSubordinateComma` is `Punctuation`-kind too, and
  filtering that whole kind silently drops the comma class from its own score.
"""

import os
import argparse
import collections
import pathlib
import re
import shutil
import subprocess
import tempfile

# harper-cli comes from a checkout of Dronakurl/harper (HARPER_CLI); the German module is the one
# `just build-wasm` made, unless HARPER_DE_WASM names another.
PLUGIN_ARGS = [
    "--plugin",
    os.environ.get("HARPER_DE_WASM", "target/wasm32-unknown-unknown/release/harper_german.wasm"),
]

HARPER = os.environ.get("HARPER_CLI", "../harper/target/release/harper-cli")

IGNORE_HARPER = {"Spaces", "CommaFixes"}


def harper_hits(rows, work):
    """Which sentences Harper reports, as {index: [rule names]}."""
    for index, (_, _, text) in enumerate(rows):
        (work / f"{index:03d}.md").write_text(text + "\n")

    result = subprocess.run(
        [HARPER, "lint", *PLUGIN_ARGS, "--format", "compact"]
        + sorted(str(path) for path in work.glob("*.md")),
        capture_output=True,
        text=True,
    )

    hits = collections.defaultdict(list)
    for line in result.stdout.splitlines():
        match = re.match(r"^(\d{3})\.md:\d+:\d+: (\w+)::(\w+):", line)
        if match and match.group(3) not in IGNORE_HARPER:
            hits[int(match.group(1))].append(match.group(3))
    return hits


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("battery", type=pathlib.Path)
    parser.add_argument(
        "--per-sentence",
        action="store_true",
        help="list every sentence with what Harper said about it",
    )
    args = parser.parse_args()

    rows = [
        line.split("\t")
        for line in args.battery.read_text().splitlines()
        if line.strip() and not line.startswith("#")
    ]

    work = pathlib.Path(tempfile.mkdtemp(prefix="battery-"))
    try:
        hits = harper_hits(rows, work)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    score = collections.defaultdict(collections.Counter)
    for index, (cls, kind, text) in enumerate(rows):
        harper = hits.get(index, [])
        if args.per_sentence:
            print(f"{kind:3} {'H+' if harper else 'H-'}  {text[:60]:<61}{','.join(harper)[:30]}")
        hit = "hit" if kind == "err" else "fp"
        total = "n" if kind == "err" else "ok"
        score[cls][f"harper_{hit}"] += 1 if harper else 0
        score[cls][f"harper_{total}"] += 1

    if args.per_sentence:
        print()

    def cell(found, total):
        return f"{found}/{total}"

    def row(name, counts):
        print(
            f"{name:<26}{cell(counts['harper_hit'], counts['harper_n']):>8}"
            f"{cell(counts['harper_fp'], counts['harper_ok']):>14}"
        )

    print(f"{'Klasse':<26}{'Harper':>8}{'Fehlalarm':>14}")
    total = collections.Counter()
    for cls in dict.fromkeys(row_[0] for row_ in rows):
        row(cls, score[cls])
        total.update(score[cls])
    print("-" * 48)
    row("GESAMT", total)


if __name__ == "__main__":
    main()
