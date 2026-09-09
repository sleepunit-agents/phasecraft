#!/usr/bin/env python3
"""Compare this finite native audition transcription with an until-stop checkout.

Usage: python3 tools/check_driving_drums_source.py /path/to/until-stop
No fetches, hardware access or generic declaration translation.
"""
import argparse
from pathlib import Path
import subprocess
import sys
import tomllib


def read(path):
    return tomllib.loads(path.read_text())


def check(source, native):
    subprocess.run(
        [sys.executable, str(source / "trace_driving_drums.py"), "--check"], check=True
    )
    piece = read(source / "run-the-line/piece.toml")
    kit = read(source / "run-the-line/kit.toml")
    audition = read(source / "run-the-line" / piece["auditions"]["driving-drums"])
    assert audition["transport"] == "isolated"
    assert audition["voices"] == ["kick", "snare"] and audition["foley"] == []
    library = read(native / "kits/909-prepared.toml")["library"]["behaviors"]
    expected = {
        "tempo": piece["tempo"], "seed": piece["seed"], "phrase_bars": 4, "parts": {}
    }
    for name in audition["voices"]:
        voice = read(source / f"run-the-line/voices/{name}.toml")
        # This transcription is one bar with step-indexed values. Refuse a changed
        # clock rather than silently assuming that the source still means 16 slots.
        assert "cycle" not in voice["trigger"], "explicit source cycle needs translation"
        assert voice["velocity"].get("per", "step") == "step"
        assert set(voice["velocity"]) <= {"pattern", "per"}
        binding = f"kit.prepared.{name}"
        output = library[binding]["output"]
        route = kit[voice["kit"]]
        assert (output["note"], output["channel"]) == (route["note"], route["channel"])
        expected["parts"][name] = {
            "use": binding,
            "trigger": {"pattern": voice["trigger"]["pattern"]},
            "accent": {"rhythm": {"steps": 16, "pulses": 0}},
            "profile": {"base": 127, "boost": 0},
            "output": {"gate_ticks": audition[name]["length"]["slots"] * 240},
            "velocity": {"pattern": voice["velocity"]["pattern"], "per": "step", "cycle": 16},
        }
    assert read(native / "compositions/driving-drums.toml") == expected, "native composition drifted from audition"
    assert read(native / "config/midi.toml")["send_clock"] is True
    print("native driving-drums transcription matches the checked source selection")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--native", type=Path, default=Path(__file__).resolve().parents[1] / "examples/run-the-line-driving-drums")
    args = parser.parse_args()
    check(args.source.resolve(), args.native.resolve())
