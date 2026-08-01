#!/usr/bin/env python3
"""Apply the committed local TextMate performance policy."""
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
report = json.loads((root / "benchmarks/textmate/theme-performance.json").read_text())
checks = [
    ("tokenization", report["tokenization"]["regressionRatio"], report["tokenization"]["gate"]),
    ("warm tokenization", report["tokenization"]["warmRegressionRatio"], report["tokenization"]["warmGate"]),
    ("cold first render", report["viewport"]["coldFirstRenderRegressionRatio"], report["viewport"]["coldFirstRenderGate"]),
    ("warm render", report["viewport"]["warmRenderRegressionRatio"], report["viewport"]["warmRenderGate"]),
    ("peak RSS", report["memory"]["rssRegressionRatio"], report["memory"]["rssGate"]),
]
for name, actual, gate in checks:
    if actual > gate:
        raise SystemExit(f"{name} regression {actual:.2%} exceeds {gate:.2%} gate")
hit_rate = report["themeCache"]["hitRate"]
if hit_rate < report["themeCache"]["gate"]:
    raise SystemExit(f"theme cache hit rate {hit_rate:.2%} is below policy")
print("ok: TextMate performance report satisfies all local gates")
