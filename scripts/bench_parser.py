"""
Benchmark the book parser — no Gutendex or Gutenberg network calls.

Measures, per text:
  1. Parser stages on their own: preprocess_text, detect_schema, build_structure_tree.
  2. A full in-process GET /api/books/structure request (ASGI transport), with
     Gutendex and the text download mocked by respx and served from the local
     file. Parser functions are wrapped with timers so the report shows how much
     of the request is parser work. Network latency is NOT included, so the
     parse share reported here is an upper bound.
  3. A cProfile of one standalone parse, top N functions by self time.

Texts are read from local files. Fetch them once with:
    mkdir -p .cache/bench
    curl -L -o .cache/bench/pg10.txt   https://www.gutenberg.org/cache/epub/10/pg10.txt
    curl -L -o .cache/bench/pg1022.txt https://www.gutenberg.org/cache/epub/1022/pg1022.txt

Run:
    uv run python scripts/bench_parser.py
    uv run python scripts/bench_parser.py --repeat 10 --top 20
"""

from __future__ import annotations

import argparse
import asyncio
import cProfile
import io
import pstats
import statistics
import sys
import time
from collections import defaultdict
from collections.abc import Callable
from pathlib import Path

import httpx
import respx

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "src"))

from metabook_py.core.cache import book_text_cache  # noqa: E402
from metabook_py.core.config import settings  # noqa: E402
from metabook_py.routers import books as books_router  # noqa: E402
from metabook_py.services import fetcher  # noqa: E402
from metabook_py.services.counter import build_structure_tree  # noqa: E402
from metabook_py.services.detector import detect_schema  # noqa: E402
from metabook_py.services.preprocessing import preprocess_text  # noqa: E402

DEFAULT_TEXTS = {
    "large": (10, ROOT / ".cache/bench/pg10.txt", "King James Bible"),
    "short": (1022, ROOT / ".cache/bench/pg1022.txt", "Walking (Thoreau essay)"),
}
TEXT_URL = "https://www.gutenberg.org/cache/epub/{id}/pg{id}.txt"


# ── Timing helpers ─────────────────────────────────────────────────────────────


def _ms(seconds: float) -> float:
    return seconds * 1000.0


def _median_ms(fn: Callable[[], object], repeat: int) -> float:
    samples = []
    for _ in range(repeat):
        t0 = time.perf_counter()
        fn()
        samples.append(time.perf_counter() - t0)
    return _ms(statistics.median(samples))


class StageTimer:
    """Accumulates wall time per label across wrapped calls."""

    def __init__(self) -> None:
        self.totals: dict[str, float] = defaultdict(float)
        self.calls: dict[str, int] = defaultdict(int)

    def wrap(self, label: str, fn: Callable) -> Callable:
        def wrapper(*args, **kwargs):
            t0 = time.perf_counter()
            try:
                return fn(*args, **kwargs)
            finally:
                self.totals[label] += time.perf_counter() - t0
                self.calls[label] += 1

        return wrapper

    def reset(self) -> None:
        self.totals.clear()
        self.calls.clear()


# ── Standalone parse ───────────────────────────────────────────────────────────


def bench_stages(raw: str, repeat: int) -> dict[str, float]:
    clean = preprocess_text(raw)
    schema = detect_schema(clean)
    out = {
        "preprocess": _median_ms(lambda: preprocess_text(raw), repeat),
        "detect": _median_ms(lambda: detect_schema(clean), repeat),
        "build": _median_ms(lambda: build_structure_tree(clean, schema), repeat),
    }
    nodes, _ = build_structure_tree(clean, schema)
    out["dump"] = _median_ms(lambda: [n.model_dump() for n in nodes], repeat)
    out["parse_total"] = out["preprocess"] + out["detect"] + out["build"]
    out["_schema"] = schema.name.value  # type: ignore[assignment]
    return out


def profile_parse(raw: str, top: int) -> str:
    prof = cProfile.Profile()
    prof.enable()
    clean = preprocess_text(raw)
    schema = detect_schema(clean)
    build_structure_tree(clean, schema)
    prof.disable()
    buf = io.StringIO()
    stats = pstats.Stats(prof, stream=buf)
    stats.strip_dirs().sort_stats(pstats.SortKey.TIME).print_stats(top)
    return buf.getvalue()


# ── Full in-process request ────────────────────────────────────────────────────


def _gutendex_payload(gid: int) -> dict:
    return {
        "count": 1,
        "results": [
            {
                "id": gid,
                "title": f"Bench {gid}",
                "authors": [],
                "languages": ["en"],
                "subjects": [],
                "formats": {"text/plain; charset=utf-8": TEXT_URL.format(id=gid)},
            }
        ],
    }


async def _bench_request(
    gid: int, raw_bytes: bytes, repeat: int, *, cold: bool, timer: StageTimer
) -> tuple[float, dict[str, float]]:
    from metabook_py.main import app

    samples: list[float] = []
    stage_sums: dict[str, list[float]] = defaultdict(list)

    with respx.mock(assert_all_called=False) as mock:
        mock.get(f"{settings.gutendex_base_url}/books/").mock(
            return_value=httpx.Response(200, json=_gutendex_payload(gid))
        )
        mock.get(TEXT_URL.format(id=gid)).mock(return_value=httpx.Response(200, content=raw_bytes))
        async with httpx.AsyncClient(
            transport=httpx.ASGITransport(app=app), base_url="http://bench"
        ) as client:
            # Warm-up (imports, first-call costs); also primes the text cache.
            r = await client.get("/api/books/structure", params={"gutenberg_id": gid})
            r.raise_for_status()
            for _ in range(repeat):
                if cold:
                    book_text_cache.clear()
                timer.reset()
                t0 = time.perf_counter()
                r = await client.get("/api/books/structure", params={"gutenberg_id": gid})
                body = r.content  # force full body
                samples.append(time.perf_counter() - t0)
                r.raise_for_status()
                assert body
                for label, total in timer.totals.items():
                    stage_sums[label].append(total)

    stages = {k: _ms(statistics.median(v)) for k, v in stage_sums.items()}
    return _ms(statistics.median(samples)), stages


def bench_request(gid: int, raw_bytes: bytes, repeat: int, *, cold: bool) -> tuple[float, dict]:
    timer = StageTimer()
    # Patch the names the request path actually calls.
    originals = (
        fetcher.preprocess_text,
        books_router.preprocess_text,
        books_router.detect_schema,
        books_router.build_structure_tree,
    )
    rate = settings.rate_limit_per_second
    fetcher.preprocess_text = timer.wrap("fetch.preprocess", fetcher.preprocess_text)
    books_router.preprocess_text = timer.wrap("router.preprocess", books_router.preprocess_text)
    books_router.detect_schema = timer.wrap("detect (incl. preprocess)", detect_schema)
    books_router.build_structure_tree = timer.wrap("build (incl. preprocess)", build_structure_tree)
    settings.rate_limit_per_second = 1e9  # the 1 req/s limiter would dominate cold runs
    try:
        return asyncio.run(_bench_request(gid, raw_bytes, repeat, cold=cold, timer=timer))
    finally:
        (
            fetcher.preprocess_text,
            books_router.preprocess_text,
            books_router.detect_schema,
            books_router.build_structure_tree,
        ) = originals
        settings.rate_limit_per_second = rate
        book_text_cache.clear()


# ── Report ─────────────────────────────────────────────────────────────────────


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--large", type=Path, default=DEFAULT_TEXTS["large"][1])
    ap.add_argument("--short", type=Path, default=DEFAULT_TEXTS["short"][1])
    ap.add_argument("--repeat", type=int, default=5)
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("--no-request", action="store_true", help="skip the full-request benchmark")
    args = ap.parse_args()

    texts = {
        "large": (DEFAULT_TEXTS["large"][0], args.large, DEFAULT_TEXTS["large"][2]),
        "short": (DEFAULT_TEXTS["short"][0], args.short, DEFAULT_TEXTS["short"][2]),
    }
    missing = [str(p) for _, p, _ in texts.values() if not p.exists()]
    if missing:
        print("Missing cached text files:\n  " + "\n  ".join(missing), file=sys.stderr)
        print("See the module docstring for the curl commands.", file=sys.stderr)
        return 1

    print(f"Python {sys.version.split()[0]} · repeat={args.repeat} (medians)\n")
    for key, (gid, path, title) in texts.items():
        raw_bytes = path.read_bytes()
        raw = raw_bytes.decode("utf-8")
        print("=" * 78)
        print(f"{key.upper()}: {title} (PG {gid}) · {len(raw_bytes) / 1024:,.0f} KB")
        print("=" * 78)

        s = bench_stages(raw, args.repeat)
        print(f"schema detected:          {s['_schema']}")
        print("standalone parse (ms):")
        for label in ("preprocess", "detect", "build"):
            print(f"  {label:<22}{s[label]:>10.1f}")
        print(f"  {'= parse total':<22}{s['parse_total']:>10.1f}")
        print(f"  {'(model_dump of nodes)':<22}{s['dump']:>10.1f}")

        if not args.no_request:
            for cold in (True, False):
                total, stages = bench_request(gid, raw_bytes, args.repeat, cold=cold)
                parse_in_req = sum(stages.values())
                tag = "cold (text cache miss)" if cold else "warm (text cached)"
                print(f"\nfull request, {tag}: {total:,.1f} ms")
                for label, ms in stages.items():
                    print(f"  {label:<28}{ms:>10.1f} ms")
                print(
                    f"  {'parser work in request':<28}{parse_in_req:>10.1f} ms"
                    f"  = {parse_in_req / total:.0%} of request"
                )

        print(f"\ncProfile, standalone parse, top {args.top} by self time:")
        print(profile_parse(raw, args.top))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
