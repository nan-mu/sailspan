#!/usr/bin/env python3
"""Check both directions on the complete selected register interface."""

import argparse
from pathlib import Path
import subprocess


EXPECTED = {
    "baseline implies ISA": "unsat",
    "ISA implies baseline (total input coverage)": "unsat",
    "baseline implies normalized": "unsat",
    "normalized implies baseline": "unsat",
    "renamed input changes the relation": "unsat",
    "missing r2 connection allows incorrect output": "sat",
    "B exit in place of B entry allows incorrect output": "sat",
    "A entry in place of A exit allows incorrect output": "sat",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("queries", type=Path)
    args = parser.parse_args()
    run = subprocess.run(
        ["z3", "-smt2", str(args.queries)], text=True, capture_output=True
    )
    print(run.stdout, end="")
    if run.returncode or run.stderr:
        raise SystemExit(f"Z3 failed with exit {run.returncode}\n{run.stderr}")
    expected = [line for name, status in EXPECTED.items() for line in (name, status)]
    # A model follows each SAT answer and is retained in the check log.
    answers = [line for line in run.stdout.splitlines() if line in EXPECTED or
               line in {"sat", "unsat", "unknown"} or line.startswith("(error")]
    if answers != expected:
        raise SystemExit("Unexpected query status or check order")


if __name__ == "__main__":
    main()
