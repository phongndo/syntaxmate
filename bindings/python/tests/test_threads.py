"""The GIL is released while highlighting, and sharing a Highlighter is safe."""

from __future__ import annotations

import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import syntaxmate

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "tests/fixtures/textmate"
SOURCES = {
    "rust": (FIXTURES / "rust/stress.rs").read_text(encoding="utf-8"),
    "python": (FIXTURES / "python/stress.py").read_text(encoding="utf-8"),
    "javascript": (FIXTURES / "javascript/stress.js").read_text(encoding="utf-8"),
}


def test_shared_highlighter_gives_identical_results():
    hl = syntaxmate.Highlighter()
    expected = {lang: hl.html(src, lang) for lang, src in SOURCES.items()}
    jobs = [lang for lang in SOURCES for _ in range(8)]
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(lambda lang: (lang, hl.html(SOURCES[lang], lang)), jobs))
    assert all(html == expected[lang] for lang, html in results)


def test_python_runs_while_highlighting():
    # While one thread is inside a long call, another thread keeps sampling the
    # clock. Holding the GIL for the call would leave a gap about as long as
    # the call; releasing it leaves only scheduling-sized gaps.
    lines = SOURCES["rust"].split("\n")
    # Unique lines defeat the line-result cache so the call stays long.
    source = "\n".join(f"{line} // {i}" for i in range(100) for line in lines)
    window = []
    started = threading.Event()

    def work():
        hl = syntaxmate.Highlighter()
        hl.html("fn warm() {}", "rust")
        started.set()
        before = time.perf_counter()
        hl.html(source, "rust")
        window.extend((before, time.perf_counter()))

    worker = threading.Thread(target=work)
    worker.start()
    started.wait()
    samples = []
    while worker.is_alive():
        samples.append(time.perf_counter())
    worker.join()
    start, end = window
    inside = [start] + [t for t in samples if start < t < end] + [end]
    longest_gap = max(b - a for a, b in zip(inside, inside[1:]))
    assert end - start > 0.02, "call too short to observe"
    assert longest_gap < 0.5 * (end - start)


def test_session_is_serialized_across_threads():
    hl = syntaxmate.Highlighter()
    session = hl.session("rust")
    lines = SOURCES["rust"].split("\n")
    expected = [session.line(line).style_ids.tolist() for line in lines[:50]]

    def replay(_):
        return [session.line("let x = 1;").lengths.tolist() for _ in range(50)]

    with ThreadPoolExecutor(max_workers=4) as pool:
        assert len(list(pool.map(replay, range(8)))) == 8
    session.reset()
    assert [session.line(line).style_ids.tolist() for line in lines[:50]] == expected
