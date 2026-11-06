#!/usr/bin/env python3
"""Measure a change to a German rule on 10 000+ sentences, in a few minutes.

    scripts/measure_rules.py fetch DIR [LABELLED.tsv]   # build DIR/corpus.tsv and DIR/labels.tsv
    CARGO_PROFILE_RELEASE_LTO=false cargo build --release --example lint_tsv
    ./target/release/examples/lint_tsv DIR/corpus.tsv > DIR/base.jsonl     # before the change
    ... change the rule, build again ...
    ./target/release/examples/lint_tsv DIR/corpus.tsv > DIR/run.jsonl      # after
    scripts/measure_rules.py score DIR run.jsonl base.jsonl [--rules=A,B]

The corpus: the training part of UD_German-GSD (edited web and news prose, where nearly every
report is a false alarm), and optionally a labelled list of your own, one sentence per line as
`err|ok<TAB>start<TAB>end<TAB>sentence` (start and end mark the mistake, -1 for a correct
sentence). Read a false alarm before counting it.
"""

import collections
import json
import sys
import urllib.request
from pathlib import Path

GSD = "https://raw.githubusercontent.com/UniversalDependencies/UD_German-GSD/master/de_gsd-ud-train.conllu"
TYPOGRAPHY = {"Spaces", "CommaFixes"}  # would count as a hit on the right sentence for the wrong reason


def fetch(out: Path, labelled: Path | None = None) -> None:
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    if labelled:
        for line in open(labelled):
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 4 and parts[0] in ("err", "ok"):
                rows.append((parts[0], int(parts[1]), int(parts[2]), parts[3]))
    with open(out / "corpus.tsv", "w") as corpus, open(out / "labels.tsv", "w") as meta:
        for i, (kind, start, end, text) in enumerate(rows):
            corpus.write(f"{i:05d}\t{text}\n")
            meta.write(f"{i:05d}\t{kind}\t{start}\t{end}\t{text}\n")
        n = 0
        for line in urllib.request.urlopen(GSD).read().decode().splitlines():
            if line.startswith("# text = "):
                corpus.write(f"gs{n:05d}\t{line[9:].strip()}\n")
                n += 1
    print(f"{len(rows)} labelled sentences, {n} treebank sentences in {out}")


def load(path):
    d = collections.defaultdict(list)
    for line in open(path):
        j = json.loads(line)
        d[j["id"]].append(j)
    return d


def score(directory: Path, run_file, base_file=None, rules=()):
    meta = {}
    for line in open(directory / "labels.tsv"):
        p = line.rstrip("\n").split("\t")
        if len(p) >= 5:
            meta[p[0]] = (p[1], int(p[2]), int(p[3]))
    text = {}
    for line in open(directory / "corpus.tsv"):
        i, t = line.rstrip("\n").split("\t", 1)
        text[i] = t

    def tally(run):
        found = errors = false_alarms = correct = 0
        for i, (kind, start, end) in meta.items():
            lints = [x for x in run.get(i, []) if x["rule"] not in TYPOGRAPHY]
            if kind == "err":
                errors += 1
                found += any(x["start"] <= end and x["end"] >= start for x in lints)
            else:
                correct += 1
                false_alarms += bool(lints)
        return found, errors, false_alarms, correct

    # The runs are named as in the usage above: inside DIR, unless the path says otherwise.
    resolve = lambda f: f if Path(f).exists() else directory / f
    run_file = resolve(run_file)
    base_file = base_file and resolve(base_file)
    run = load(run_file)
    found, errors, fa, correct = tally(run)
    print(f"Labelled sentences: errors found {found}/{errors}, correct sentences with a report {fa}/{correct}")
    if base_file:
        base = load(base_file)
        key = lambda x: (x["id"], x["rule"], x["start"], x["end"])
        a = {key(x): x for v in run.values() for x in v}
        b = {key(x): x for v in base.values() for x in v}
        new = [a[k] for k in a if k not in b]
        lost = [b[k] for k in b if k not in a]
        print(f"before: found {tally(base)[0]}, false-alarm sentences {tally(base)[2]}")
        print(f"new reports {len(new)}: {dict(collections.Counter(x['rule'] for x in new))}")
        print(f"lost reports {len(lost)}: {dict(collections.Counter(x['rule'] for x in lost))}")
        for x in new:
            if not rules or x["rule"] in rules:
                kind = meta.get(x["id"], ("treebank",))[0]
                print(f"  + [{kind}] {x['rule']} »{x['problem']}« :: {text[x['id']][:140]}")


if __name__ == "__main__":
    if len(sys.argv) >= 3 and sys.argv[1] == "fetch":
        fetch(Path(sys.argv[2]), Path(sys.argv[3]) if len(sys.argv) >= 4 else None)
    elif len(sys.argv) >= 4 and sys.argv[1] == "score":
        extra = [a for a in sys.argv[4:] if not a.startswith("--rules=")]
        wanted = [a[8:].split(",") for a in sys.argv[4:] if a.startswith("--rules=")]
        score(Path(sys.argv[2]), sys.argv[3], extra[0] if extra else None, set(wanted[0]) if wanted else ())
    else:
        print(__doc__)
