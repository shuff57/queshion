#!/usr/bin/env python3
"""Compare verSHion's figure scale numbers with the reference generators.

Usage (from the repo root):
    python3 tools/check_figures.py                 # all four subjects
    python3 tools/check_figures.py physics biology # just those
    python3 tools/check_figures.py --labels        # also compare non-numeric text

Runs `cargo run --example dump_svgs` for verSHion's default SVGs, fetches each
reference page (chemistryfigures.com etc.), reads the text in its `div.sheet`
preview SVG, and reports numbers that are missing or extra. Writes a JSON
report to /tmp/figure_parity.json.
"""
import argparse
import glob
import json
import os
import re
import subprocess
import urllib.request
from bs4 import BeautifulSoup

SITES = {
    "chemistry": ["bohr-model", "gas-syringe", "heating-cooling-curve", "length-reading",
                  "lewis-structures", "line-spectrum", "mass-reading", "mass-spectrum",
                  "orbital-diagram", "particle-diagram", "photoelectron-spectrum", "ph-reading",
                  "temperature-reading", "titration-curve", "volume-by-displacement", "volume-reading"],
    "physics": ["circuit-diagram", "free-body-diagram", "inclined-plane", "motion-graphs",
                "projectile-motion", "pulley", "spring-scale", "vector-diagram", "waves"],
    "biology": ["gel-electrophoresis", "micropipette-reading", "pedigree", "population-growth",
                "predator-prey", "punnett-square"],
    "math": ["angles", "box-plot", "composite-shape", "cone", "coordinate-grid", "cylinder",
             "kite", "length-reading", "mapping-diagram", "number-line", "parallel-lines",
             "parallelogram", "prism", "pyramid", "rectangle", "regular-polygon", "sphere",
             "trapezoid", "triangle"],
}


def texts(svg):
    soup = BeautifulSoup(svg, "html.parser")
    return [t.get_text(" ", strip=True) for t in soup.find_all("text")]


def numbers(items):
    out = set()
    for item in items:
        for m in re.findall(r"-?\d+(?:\.\d+)?", item):
            out.add(round(float(m), 3))
    return out


def fmt(values):
    return ", ".join("%g" % v for v in sorted(values))


def reference_text(site, slug):
    url = f"https://{site}figures.com/{slug}"
    req = urllib.request.Request(url, headers={"User-Agent": "verSHion-parity/1.0"})
    html = urllib.request.urlopen(req, timeout=30).read().decode("utf-8", "replace")
    soup = BeautifulSoup(html, "html.parser")
    sheet = soup.select_one("div.sheet")
    svg = sheet.find("svg") if sheet else None
    return (texts(str(svg)) if svg else []), url


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("subjects", nargs="*", default=[])
    ap.add_argument("--labels", action="store_true", help="also compare non-numeric labels")
    ap.add_argument("--json", default="/tmp/figure_parity.json")
    args = ap.parse_args()
    subjects = args.subjects or list(SITES)
    for s in subjects:
        if s not in SITES:
            ap.error(f"unknown subject {s!r}; choose from {', '.join(SITES)}")

    subprocess.run(["cargo", "run", "--quiet", "--example", "dump_svgs", "--", "/tmp/vs_svgs"], check=True)
    vs = {os.path.basename(p)[:-4]: texts(open(p).read()) for p in glob.glob("/tmp/vs_svgs/*.svg")}

    report, differing, total = {}, 0, 0
    for subject in subjects:
        print(f"\n### {subject}")
        for slug in SITES[subject]:
            total += 1
            try:
                rt, url = reference_text(subject, slug)
            except Exception as e:
                print(f"  {slug}: REFERENCE ERROR {e}")
                continue
            vt = vs.get(slug)
            if vt is None:
                print(f"  {slug}: no verSHion figure with this id")
                continue
            rn, vn = numbers(rt), numbers(vt)
            entry = {"url": url, "ref_numbers": sorted(rn), "vs_numbers": sorted(vn),
                     "missing": sorted(rn - vn), "extra": sorted(vn - rn)}
            if args.labels:
                rtext = {t for t in rt if not re.fullmatch(r"-?\d+(\.\d+)?", t.strip())}
                vtext = {t for t in vt if not re.fullmatch(r"-?\d+(\.\d+)?", t.strip())}
                # The reference puts a value and its unit/degree sign in separate text
                # nodes ("24" and "°"); we use one node ("24°"). Not a visible difference,
                # so a reference token is present when it is a substring of any of our nodes.
                entry["ref_text_missing"] = sorted(t for t in rtext - vtext if not any(t.strip() in v for v in vt))
            report[f"{subject}/{slug}"] = entry
            if rn == vn and not entry.get("ref_text_missing"):
                print(f"  ok   {slug}")
                continue
            differing += 1
            print(f"  DIFF {slug}")
            if rn - vn:
                print(f"       missing: {fmt(rn - vn)}")
            if vn - rn:
                print(f"       extra:   {fmt(vn - rn)}")
            if entry.get("ref_text_missing"):
                print(f"       ref text not in vs: {entry['ref_text_missing'][:6]}")

    json.dump(report, open(args.json, "w"), indent=1)
    print(f"\n{differing} of {total} figures differ; report: {args.json}")


if __name__ == "__main__":
    main()
