#!/usr/bin/env python3
"""Count words in random German Wikipedia articles and write the most frequent ones.

    scripts/build_word_frequency.py --articles 3000 --top 15000 data/word_frequency.txt

The spell checker uses the list only to order suggestions that are equally close
to a misspelling (*git* is more likely *gibt* than *Gift*): one word per line,
the most frequent first. Counting words gives facts, not text, so nothing of the
articles is kept; the downloaded extracts are cached in `--cache` (not committed).

Random articles, not the topic list of `fetch_german_corpus.py`, so that the
corpus used to measure false alarms stays independent of the list.
"""

import argparse
import collections
import json
import pathlib
import re
import time
import urllib.parse
import urllib.request

API = "https://de.wikipedia.org/w/api.php"
UA = "harper-german-frequency/1.0 (https://github.com/Automattic/harper)"
WORD = re.compile(r"[A-Za-zÄÖÜäöüß]+(?:-[A-Za-zÄÖÜäöüß]+)*")


def get(params):
    url = API + "?" + urllib.parse.urlencode({**params, "format": "json"})
    request = urllib.request.Request(url, headers={"User-Agent": UA})
    for attempt in range(5):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        except Exception:
            time.sleep(5 * (attempt + 1))
    raise RuntimeError(url)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=pathlib.Path)
    parser.add_argument("--articles", type=int, default=3000)
    parser.add_argument("--top", type=int, default=15000)
    parser.add_argument("--cache", type=pathlib.Path, default=pathlib.Path(".archive/frequency"))
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)

    have = len(list(args.cache.glob("*.txt")))
    while have < args.articles:
        data = get({
            "action": "query", "generator": "random", "grnnamespace": 0, "grnlimit": 20,
            "prop": "extracts", "explaintext": 1, "exlimit": 20, "exintro": 0,
        })
        for page in data.get("query", {}).get("pages", {}).values():
            text = page.get("extract") or ""
            if len(text) > 500:
                (args.cache / f"{page['pageid']}.txt").write_text(text)
        have = len(list(args.cache.glob("*.txt")))
        time.sleep(0.5)

    counts = collections.Counter()
    for path in args.cache.glob("*.txt"):
        for word in WORD.findall(path.read_text()):
            if len(word) > 1:
                counts[word] += 1
    # A word and its capitalized form count together, under the spelling seen more often.
    folded = collections.Counter()
    spelling = {}
    for word, n in counts.items():
        key = word.lower()
        folded[key] += n
        if counts[spelling.get(key, word)] <= n:
            spelling[key] = word
    words = [spelling[key] for key, n in folded.most_common() if n >= 3][: args.top]
    args.output.write_text("\n".join(words) + "\n")
    print(f"{len(words)} words from {have} articles")


if __name__ == "__main__":
    main()
