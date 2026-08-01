"""Compact Python grammar sample with café and astral glyphs 🚀 𝄞."""
from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Signal:
    name: str
    strength: float = 1.0

    def label(self) -> str:
        return f"{self.name!r}: {self.strength:.1%}"


def visible(signals: list[Signal], floor: float = 0.25) -> dict[str, float]:
    """Return readings at or above the requested floor."""
    return {
        signal.name: signal.strength
        for signal in signals
        if signal.strength >= floor
    }


if __name__ == "__main__":
    samples = [Signal("café 🚀", 0.875), Signal("𝄞", 0.125)]
    for name, strength in visible(samples).items():
        print(f"{name} -> {strength=:.2f}")
