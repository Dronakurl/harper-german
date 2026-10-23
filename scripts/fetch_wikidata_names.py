#!/usr/bin/env python3
"""Fetch the best known German proper names from Wikidata items and write `wikidata_names.tsv`.

    fetch_wikidata_names.py [--out data/wikidata_names.tsv]

Wikidata items are CC0, so the result may be committed with its source named.
The queries run against QLever's copy of Wikidata (https://qlever.cs.uni-freiburg.de), which
answers each of them in seconds; the public Wikidata Query Service stops after 60 seconds and
then sends what it has so far, which once left Flensburg, Minsk and Ankara out of the list.
They ask for the German label of

* countries, subdivisions, rivers, mountains, islands, seas and the like, and places with at
  least 15 000 inhabitants, ranked by sitelinks (how many Wikipedias have an article);
* what the people of a place are called (*Lübecker*, *Berner*);
* given and family names, ranked by how many people with an article in the German Wikipedia
  bear them: that is what German text mentions, where sitelinks favour names of other
  languages (*Jörg* has 6 sitelinks, and 1147 bearers);
* the family names of the best known people (*Putin*, *Baerbock*), by the sitelinks of the
  best known bearer;
* organisations, companies and clubs.

Each kind has a budget (BUDGET), filled from the top of its ranking. Only one-word labels of
four letters or more are kept (shorter ones are typos as often as names). The file has one
`name<TAB>kind` per line; a name found by several queries keeps the kind of the first.
"""

import argparse
import pathlib
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

ENDPOINT = "https://qlever.cs.uni-freiburg.de/api/wikidata"
UA = "harper-german-dictionary/1.0 (https://github.com/Dronakurl/harper-german)"
PREFIXES = (
    "PREFIX wd: <http://www.wikidata.org/entity/> PREFIX wdt: <http://www.wikidata.org/prop/direct/> "
    "PREFIX wikibase: <http://wikiba.se/ontology#> PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#> "
    "PREFIX schema: <http://schema.org/> "
)

# How many names of each kind the word list may carry.
BUDGET = {"geo": 2500, "place": 8000, "demonym": 2000, "given": 4000, "family": 6000, "person": 5000, "org": 1500}

# People with an article in the German Wikipedia.
DEWIKI_PEOPLE = "?a schema:about ?x . ?a schema:isPartOf <https://de.wikipedia.org/> . "

# In this order: a name several queries return keeps the kind of the first.
QUERIES = {
    "geo": 'SELECT ?l ?s WHERE { VALUES ?c { wd:Q6256 wd:Q3624078 wd:Q10864048 wd:Q1221156 wd:Q35657 wd:Q5107 '
           'wd:Q46831 wd:Q4022 wd:Q8502 wd:Q23442 wd:Q165 wd:Q23397 wd:Q82794 wd:Q15324 wd:Q39816 wd:Q1620908 } '
           '?x wdt:P31 ?c . ?x wikibase:sitelinks ?s . ?x rdfs:label ?l . FILTER(?s>=10) FILTER(LANG(?l)="de") }',
    "place": 'SELECT ?l ?s WHERE { ?x wdt:P1082 ?p . FILTER(?p>=15000) ?x wikibase:sitelinks ?s . '
             '?x rdfs:label ?l . FILTER(LANG(?l)="de") }',
    "demonym": 'SELECT ?l (MAX(?s) AS ?m) WHERE { ?x wdt:P1549 ?l . FILTER(LANG(?l)="de") '
               '?x wikibase:sitelinks ?s . } GROUP BY ?l',
    "given": 'SELECT ?l (COUNT(DISTINCT ?x) AS ?n) WHERE { ?x wdt:P735 ?g . ' + DEWIKI_PEOPLE +
             '?g rdfs:label ?l . FILTER(LANG(?l)="de") } GROUP BY ?l',
    "family": 'SELECT ?l (COUNT(DISTINCT ?x) AS ?n) WHERE { ?x wdt:P734 ?g . ' + DEWIKI_PEOPLE +
              '?g rdfs:label ?l . FILTER(LANG(?l)="de") } GROUP BY ?l',
    "person": 'SELECT ?l (MAX(?s) AS ?m) WHERE { ?x wdt:P31 wd:Q5 . ?x wikibase:sitelinks ?s . FILTER(?s>=20) '
              '?x wdt:P734 ?g . ?g rdfs:label ?l . FILTER(LANG(?l)="de") } GROUP BY ?l',
    "org": 'SELECT ?l ?s WHERE { VALUES ?c { wd:Q4830453 wd:Q783794 wd:Q43229 wd:Q891723 wd:Q327333 wd:Q7278 '
           'wd:Q484652 } ?x wdt:P31 ?c . ?x wikibase:sitelinks ?s . ?x rdfs:label ?l . FILTER(?s>=25) '
           'FILTER(LANG(?l)="de") }',
}


def run(query):
    """The (label, rank) pairs the query returns."""
    url = ENDPOINT + "?" + urllib.parse.urlencode({"query": PREFIXES + query})
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "text/tab-separated-values"})
    for attempt in range(6):
        try:
            with urllib.request.urlopen(req, timeout=180) as resp:
                text = resp.read().decode("utf-8")
            # A server that gives up half way still answers 200, with an error after the rows.
            if "Exception" in text or not text.startswith("?l\t"):
                raise urllib.error.HTTPError(url, 500, "incomplete answer", None, None)
            break
        except urllib.error.HTTPError as err:
            # The service rate-limits and times out under load; wait and ask again.
            if err.code not in (429, 500, 502, 503, 504) or attempt == 5:
                raise
            print(f"HTTP {err.code}, waiting", file=sys.stderr)
            time.sleep(90)
    out = []
    for line in text.splitlines()[1:]:
        label, _, links = line.partition("\t")
        if label.startswith('"') and label.endswith('"@de'):
            try:
                out.append((label[1:-4], int(float(links.strip('"').split("^^")[0].strip('"')))))
            except ValueError:
                out.append((label[1:-4], 0))
    return out


def keep(label):
    return (
        len(label) >= 4
        and label[0].isupper()
        and all(c.isalpha() or c == "-" for c in label)
        and not label.endswith("-")
    )


def main():
    here = pathlib.Path(__file__).resolve().parent
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(here.parent / "data" / "wikidata_names.tsv"))
    ap.add_argument("--only", nargs="*", help="ask for these kinds only; the rest of an existing file is kept")
    args = ap.parse_args()
    rows = {}  # label -> kind
    old = pathlib.Path(args.out)
    if args.only and old.exists():
        for line in old.read_text(encoding="utf-8").splitlines():
            name, _, kind = line.partition("\t")
            if kind and not line.startswith("#") and kind not in args.only:
                rows[name] = kind
    for kind, query in QUERIES.items():
        if args.only and kind not in args.only:
            continue
        best = {}
        try:
            got = run(query)
        except urllib.error.URLError as err:  # the service refuses now and then: keep what the other kinds found
            print(f"{kind}: skipped ({err})", file=sys.stderr)
            continue
        for label, links in got:
            if keep(label):
                best[label] = max(links, best.get(label, 0))
        ranked = sorted(best, key=lambda w: (-best[w], w))[: BUDGET[kind]]
        for label in ranked:
            rows.setdefault(label, kind)
        print(f"{kind}: {len(best)} labels, {len(ranked)} kept", file=sys.stderr)
        time.sleep(2)
    header = (
        "# German proper names from Wikidata items (CC0, https://www.wikidata.org/).\n"
        "# Written by scripts/fetch_wikidata_names.py; one name and its kind per line.\n"
    )
    with open(args.out, "w", encoding="utf-8") as fh:
        fh.write(header)
        for label in sorted(rows, key=lambda w: (w.casefold(), w)):
            fh.write(f"{label}\t{rows[label]}\n")
    print(f"wrote {len(rows)} names to {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
