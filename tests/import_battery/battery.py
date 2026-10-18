#!/usr/bin/env python3
"""verSHion import test battery: PDF -> pages -> live AI -> app parser -> score vs gold.

  python3 tests/import_battery/battery.py --pdf ~/Downloads/Ch4QuizKey.pdf --runs 3
Needs: pdftoppm, ImageMagick `convert`, a saved AI key (Settings), target/release/vershion.
Never reads or prints the key; the app binary does the network call itself.
"""
import argparse, concurrent.futures as cf, difflib, json, os, re, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).parent
ROOT = HERE.parent.parent


def sh(cmd, env=None, timeout=360):
    e = dict(os.environ); e.update(env or {})
    return subprocess.run(cmd, capture_output=True, text=True, env=e, timeout=timeout)


def norm(s):
    s = re.sub(r"^\s*\d+[.)]\s*", "", s or "")
    return re.sub(r"\s+", " ", re.sub(r"[^a-z0-9. ]", " ", s.lower())).strip()


def sim(a, b):
    a, b = norm(a)[:200], norm(b)[:200]
    return difflib.SequenceMatcher(None, a, b).ratio()


def kind_of(q):
    k = q["kind"]
    return k if isinstance(k, str) else next(iter(k))


def blob(q):
    return json.dumps(q["kind"]).lower()


def box_iou(a, b):
    try:
        ix = max(0, min(a[2], b[2]) - max(a[0], b[0])); iy = max(0, min(a[3], b[3]) - max(a[1], b[1]))
        inter = ix * iy; u = (a[2]-a[0])*(a[3]-a[1]) + (b[2]-b[0])*(b[3]-b[1]) - inter
        return inter / u if u > 0 else 0
    except Exception:
        return 0


def make_pages(pdf, out):
    out.mkdir(parents=True, exist_ok=True)
    r = sh(["pdftoppm", "-png", "-r", "150", str(pdf), str(out / "page")])
    pages = sorted(out.glob("page-*.png"))
    return pages, r.returncode


def make_controls(pages, out):
    out.mkdir(parents=True, exist_ok=True)
    c = {}
    c["blank"] = out / "blank.png"; sh(["convert", "-size", "1240x1600", "xc:white", str(c["blank"])])
    apple = next(iter(Path.home().glob("Downloads/close-up-of-a-green-apple*.webp")), None)
    if apple:
        c["photo_no_text"] = out / "photo.png"; sh(["convert", str(apple), str(c["photo_no_text"])])
    c["rotated_90"] = out / "rot90.png"; sh(["convert", str(pages[0]), "-rotate", "90", str(c["rotated_90"])])
    c["low_res"] = out / "lowres.png"; sh(["convert", str(pages[1]), "-resize", "30%", str(c["low_res"])])
    c["top_half_only"] = out / "tophalf.png"; sh(["convert", str(pages[1]), "-gravity", "North", "-crop", "100%x50%+0+0", "+repage", str(c["top_half_only"])])
    return c


def live(binary, img, out_txt, out_json, page_no=1):
    t0 = time.time()
    r = sh([binary], {"VERSHION_CONVERT": str(img)})
    res = {"convert_ok": r.returncode == 0, "secs": round(time.time() - t0, 1), "err": r.stderr.strip()[:200]}
    out_txt.write_text(r.stdout)
    if r.returncode != 0:
        res["parse_ok"] = False; return res
    store = out_txt.parent / "store"; store.mkdir(exist_ok=True)
    p = sh([binary], {"VERSHION_PARSE": str(out_txt), "VERSHION_PAGE": str(img), "VERSHION_PAGE_NO": str(page_no), "XDG_DATA_HOME": str(store)})
    res["parse_ok"] = p.returncode == 0
    if p.returncode == 0:
        out_json.write_text(p.stdout); d = json.loads(p.stdout)
        res["questions"] = d["questions"]; res["warnings"] = d["warnings"]; res["figures"] = d["figures"]
        res["store"] = d["store"]
        try: res["raw"] = json.loads(r.stdout[r.stdout.find("{"):r.stdout.rfind("}")+1])
        except Exception: res["raw"] = None
    else:
        res["parse_err"] = p.stderr.strip()[:200]
    return res


def score_page(gold, res):
    """Returns (checks, lines). check = (name, tier, ok, detail). tier HARD|SOFT."""
    checks = []
    def add(n, tier, ok, d=""): checks.append((n, tier, bool(ok), d))
    add("converted", "HARD", res["convert_ok"], res.get("err", ""))
    add("parser accepts the page", "HARD", res.get("parse_ok"), res.get("parse_err", ""))
    qs = res.get("questions") or []
    add(f"question count == {len(gold)}", "HARD", len(qs) == len(gold), f"got {len(qs)}")
    used, order = set(), []
    for g in gold:
        best, bs = None, 0
        for i, q in enumerate(qs):
            if i in used: continue
            s = max(sim(g["stem"], q["stem"]), sim(g["stem"], q["stem"].split("\n")[0]))
            if s > bs: best, bs = i, s
        n = g["num"]
        if best is None or bs < 0.6:
            add(f"{n} found", "HARD", False, f"best match {bs:.2f}"); continue
        used.add(best); order.append(best); q = qs[best]
        add(f"{n} found", "HARD", True)
        add(f"{n} stem text ≥0.90", "SOFT", bs >= 0.90, f"{bs:.2f}")
        if kind_of(q) == "Blank":
            acc = q["kind"]["Blank"]["accept"]
            add(f"{n} one answer list per ___ ({q['stem'].count('___')})", "HARD", len(acc) == q["stem"].count("___") and all(acc), f"{len(acc)} lists, empty: {sum(1 for a in acc if not a)}")
        add(f"{n} kind in {g['kinds']}", "SOFT", kind_of(q) in g["kinds"], kind_of(q))
        if "choices" in g:
            ch = [a["text"].strip() for a in q["kind"]["Mc"]["answers"]] if kind_of(q) == "Mc" else []
            ok = [c.lower() for c in ch] == [c.lower() for c in g["choices"]]
            add(f"{n} choices exact+ordered", "HARD", ok, str(ch))
            cor = [a["text"].strip() for a in q["kind"]["Mc"]["answers"] if a.get("correct")] if kind_of(q) == "Mc" else []
            add(f"{n} correct answer", "HARD", [c.lower() for c in cor] == [c.lower() for c in g["correct"]], str(cor))
        if "answers" in g:
            b = blob(q) + " " + q["stem"].lower()
            miss = [a for a in g["answers"] if a.lower() not in b]
            add(f"{n} key answers present", "SOFT", not miss, f"missing {miss}" if miss else "")
        if g.get("needs_image"):
            add(f"{n} image EXTRACTED from page", "HARD", q.get("image") is not None, "no image attached")
            if q.get("image") and g.get("figure_box"):
                raw = (res.get("raw") or {}).get("bank", {}).get("questions", [])
                rb = next((r.get("figure", {}).get("box") for r in raw if sim(r.get("stem", ""), g["stem"]) > 0.6 and r.get("figure")), None)
                iou = box_iou(rb, g["figure_box"]) if rb else 0
                add(f"{n} figure box IoU ≥0.6 vs gold", "HARD", iou >= 0.6, f"IoU {iou:.2f} box={rb}")
        else:
            add(f"{n} no invented figure", "HARD", q.get("image") is None and "[picture" not in q["stem"].lower(),
                "figure or [picture] where the page has none")
    add("no leading question numbers in stems", "SOFT", not any(re.match(r"\s*\d+[.)]\s", q["stem"]) for q in qs))
    add("no [picture] placeholders in stems", "SOFT", not any("[picture" in q["stem"].lower() for q in qs))
    add("order preserved", "HARD", order == sorted(order), str(order))
    return checks


def score_control(name, res):
    qs = res.get("questions") or []
    exp = {"blank": 0, "photo_no_text": 0}
    if name in exp:
        # No quiz here: ideal is a clean refusal/empty result, never invented questions.
        return [(f"{name}: no invented questions", "HARD", len(qs) == 0 and True, f"{len(qs)} questions; convert_ok={res['convert_ok']} parse_ok={res.get('parse_ok')}")]
    # Degraded copies of real pages: still extract something sensible.
    return [(f"{name}: parser accepts", "HARD", res.get("parse_ok"), res.get("parse_err", res.get("err", ""))),
            (f"{name}: ≥1 question", "SOFT", len(qs) >= 1, f"{len(qs)} questions")]


def fit_stage(binary, out, runs, results):
    """Figure fitting: every cropped quiz figure must become an editable Lewis figure
    (preset acetyl chloride); a photo and a text-only crop must NOT get a fake figure."""
    checks, crops = [], []
    for tag, (key, res) in sorted(results.items()):
        for q in res.get("questions", []) if isinstance(key, int) else []:
            if q.get("image") and key in (1, 3) and res.get("store"):
                crops.append((tag, Path(res["store"]) / q["image"]["file"]))
    seen = {}
    for tag, p in crops:
        seen.setdefault(str(p), (tag, p))
    store = out / "fit_store"; (store / "vershion").mkdir(parents=True, exist_ok=True)
    jobs = [(f"{tag}#{i+1}", p, True) for tag, p in seen.values() for i in range(runs)]
    photo = out / "controls" / "photo.png"
    if photo.exists():
        jobs += [(f"photo#{i+1}", photo, False) for i in range(runs)]
    def one(j):
        name, p, want = j
        r = sh([binary], {"VERSHION_FIT": str(p), "XDG_DATA_HOME": str(store)})
        o = r.stdout.strip()
        if want:
            return (f"fit {name}: editable Lewis figure, acetyl chloride preset", "HARD", "lewis-structures" in o and "acetyl chloride" in o, o[:120])
        return (f"fit {name}: no fake figure for a photo", "HARD", o.startswith("no fit"), o[:120])
    with cf.ThreadPoolExecutor(4) as ex:
        checks = list(ex.map(one, jobs))
    return checks


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pdf", required=True)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--binary", default=str(ROOT / "target/release/vershion"))
    ap.add_argument("--out", default=str(HERE / "_run"))
    ap.add_argument("--gold", default=str(HERE / "gold_ch4.json"))
    ap.add_argument("--no-controls", action="store_true")
    ap.add_argument("--fit", action="store_true", help="also run the figure-fitting stage (extra model calls)")
    a = ap.parse_args()
    out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
    gold = json.load(open(a.gold))["pages"]
    report, allchecks = [], []

    pages, rc = make_pages(Path(a.pdf), out / "pages")
    allchecks.append(("T1 PDF splits into one image per page", "HARD", len(pages) == len(gold), f"{len(pages)} pages"))
    jobs = []
    for i, p in enumerate(pages, 1):
        for r in range(a.runs):
            jobs.append((f"page{i}_run{r+1}", i, p))
    if not a.no_controls:
        for name, p in make_controls(pages, out / "controls").items():
            jobs.append((f"control_{name}", name, p))

    results = {}
    with cf.ThreadPoolExecutor(4) as ex:
        futs = {ex.submit(live, a.binary, p, out / f"{tag}.txt", out / f"{tag}.json", key if isinstance(key, int) else 1): (tag, key) for tag, key, p in jobs}
        for f in cf.as_completed(futs):
            tag, key = futs[f]; results[tag] = (key, f.result())

    per_run_sigs = {}
    for tag in sorted(results):
        key, res = results[tag]
        if isinstance(key, int):
            cs = score_page(gold[str(key)], res)
            sig = tuple(kind_of(q) for q in res.get("questions", []))
            per_run_sigs.setdefault(key, []).append(sig)
        else:
            cs = score_control(key, res)
        report.append((tag, res.get("secs"), cs)); allchecks += [(f"{tag}: {n}", t, ok, d) for n, t, ok, d in cs]
    for pg, sigs in per_run_sigs.items():
        allchecks.append((f"page{pg}: same question kinds across {len(sigs)} runs", "SOFT", len(set(sigs)) == 1, str(sigs)))

    if not a.no_controls and a.fit:
        key_dir = out / "fit_store" / "vershion"; key_dir.mkdir(parents=True, exist_ok=True)
        src = Path.home() / ".local/share/vershion"
        for f in ("settings.json", "ai-keys.json"):
            if (src / f).exists():
                (key_dir / f).write_bytes((src / f).read_bytes()); (key_dir / f).chmod(0o600)
        try:
            fc = fit_stage(a.binary, out, a.runs, results)
        finally:
            (key_dir / "ai-keys.json").unlink(missing_ok=True)  # never leave a key copy behind
        report.append(("figure fitting", None, fc)); allchecks += [(f"fitting: {n}", t, ok, d) for n, t, ok, d in fc]

    lines = ["# verSHion import battery", f"PDF: {a.pdf}  ·  runs per page: {a.runs}", ""]
    for tag, secs, cs in report:
        bad = [c for c in cs if not c[2]]
        lines.append(f"## {tag}  ({secs}s)  {len(cs)-len(bad)}/{len(cs)} pass")
        for n, t, ok, d in cs:
            if not ok: lines.append(f"- FAIL [{t}] {n}  {d}")
        lines.append("")
    hard = [c for c in allchecks if c[1] == "HARD"]; soft = [c for c in allchecks if c[1] == "SOFT"]
    hf = [c for c in hard if not c[2]]; sf = [c for c in soft if not c[2]]
    lines += ["## Totals", f"HARD: {len(hard)-len(hf)}/{len(hard)} pass", f"SOFT: {len(soft)-len(sf)}/{len(soft)} pass"]
    (out / "report.md").write_text("\n".join(lines))
    json.dump([{"check": n, "tier": t, "ok": ok, "detail": d} for n, t, ok, d in allchecks], open(out / "report.json", "w"), indent=1)
    print("\n".join(lines))
    sys.exit(1 if hf else 0)


if __name__ == "__main__":
    main()
