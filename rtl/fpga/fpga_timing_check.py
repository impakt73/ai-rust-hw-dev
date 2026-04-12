#!/usr/bin/env python3

import argparse
import re
import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Fail the FPGA build when timing constraints are not met."
    )
    parser.add_argument(
        "--target",
        choices=(
            "ecp5_icepi_zero",
            "artix7_alchitry_au",
            "cyclonev_analogue_pocket",
            "gowin_tang_primer_25k",
        ),
        required=True,
        help="FPGA target to validate.",
    )
    parser.add_argument(
        "--build-dir",
        help="Override the build directory. Defaults to rtl/fpga/build/<target>.",
    )
    return parser.parse_args()


def fail(message: str) -> None:
    sys.stderr.write(f"ERROR: {message}\n")
    raise SystemExit(1)


def load_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def unique_existing_paths(build_dir: Path, patterns: list[str]) -> list[Path]:
    paths: list[Path] = []
    seen: set[Path] = set()
    for pattern in patterns:
        for candidate in sorted(build_dir.glob(pattern)):
            resolved = candidate.resolve()
            if not candidate.is_file() or resolved in seen:
                continue
            seen.add(resolved)
            paths.append(candidate)
    return paths


def first_matching_float(text: str, patterns: list[str]) -> float | None:
    for pattern in patterns:
        match = re.search(pattern, text, flags=re.IGNORECASE)
        if match is not None:
            return float(match.group(1))
    return None


def check_ecp5_icepi_zero(build_dir: Path) -> str:
    candidates = unique_existing_paths(
        build_dir, ["riscv_fpga_timing.rpt", "nextpnr.log"]
    )
    if not candidates:
        fail(f"No ECP5 timing artifacts found under {build_dir}")

    pattern = re.compile(
        r"Max frequency for clock\s+'([^']+)':\s+([0-9.]+)\s+MHz\s+\((PASS|FAIL)\s+at\s+([0-9.]+)\s+MHz\)"
    )
    results: list[tuple[Path, str, float, str, float]] = []
    for path in candidates:
        for match in pattern.finditer(load_text(path)):
            results.append(
                (
                    path,
                    match.group(1),
                    float(match.group(2)),
                    match.group(3),
                    float(match.group(4)),
                )
            )

    if not results:
        fail(f"Unable to parse nextpnr timing status from {', '.join(str(path) for path in candidates)}")

    failing = [result for result in results if result[3] == "FAIL"]
    if failing:
        path, clock_name, max_frequency_mhz, _, target_frequency_mhz = failing[0]
        fail(
            f"Timing failed for {clock_name} in {path}: "
            f"{max_frequency_mhz:.2f} MHz < target {target_frequency_mhz:.2f} MHz"
        )

    slowest = min(results, key=lambda result: result[2] - result[4])
    path, clock_name, max_frequency_mhz, _, target_frequency_mhz = slowest
    return (
        f"Timing PASS for {clock_name} in {path}: "
        f"{max_frequency_mhz:.2f} MHz >= target {target_frequency_mhz:.2f} MHz"
    )


def parse_vivado_wns(text: str) -> float | None:
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if "WNS(ns)" not in line:
            continue
        for candidate in lines[index + 1 : index + 8]:
            if set(candidate.strip()) <= {"-", " "}:
                continue
            numbers = re.findall(r"-?\d+(?:\.\d+)?", candidate)
            if numbers:
                return float(numbers[0])
    return None


def check_artix7_alchitry_au(build_dir: Path) -> str:
    candidates = unique_existing_paths(
        build_dir, ["riscv_fpga_timing.rpt", "riscv_fpga_timing_summary.rpt"]
    )
    if not candidates:
        fail(f"No Vivado timing artifacts found under {build_dir}")

    for path in candidates:
        text = load_text(path)
        slack = first_matching_float(
            text,
            [r"Slack \((?:MET|VIOLATED)\)\s*:\s*(-?[0-9.]+)ns"],
        )
        if slack is None:
            slack = parse_vivado_wns(text)
        if slack is None:
            continue
        if slack < 0:
            fail(f"Timing failed in {path}: worst setup slack {slack:.3f} ns")
        return f"Timing PASS in {path}: worst setup slack {slack:.3f} ns"

    fail(
        f"Unable to parse Vivado timing status from {', '.join(str(path) for path in candidates)}"
    )


def evaluate_report(
    *,
    tool_name: str,
    path: Path,
    text: str,
    pass_patterns: list[str],
    fail_patterns: list[str],
    slack_patterns: list[str],
) -> str | None:
    for pattern in fail_patterns:
        if re.search(pattern, text, flags=re.IGNORECASE):
            fail(f"Timing failed in {path} ({tool_name} reported an explicit violation)")

    slack = first_matching_float(text, slack_patterns)
    if slack is not None:
        if slack < 0:
            fail(f"Timing failed in {path}: worst setup slack {slack:.3f} ns")
        return f"Timing PASS in {path}: worst setup slack {slack:.3f} ns"

    for pattern in pass_patterns:
        if re.search(pattern, text, flags=re.IGNORECASE):
            return f"Timing PASS in {path}"

    return None


def check_cyclonev_analogue_pocket(build_dir: Path) -> str:
    candidates = unique_existing_paths(
        build_dir,
        [
            "riscv_fpga_timing.rpt",
            "riscv_fpga_timing_summary.rpt",
            "output_files/*.sta.rpt",
            "output_files/*.summary",
            "output_files/*.rpt",
        ],
    )
    if not candidates:
        fail(f"No Quartus timing artifacts found under {build_dir}")

    for path in candidates:
        result = evaluate_report(
            tool_name="Quartus",
            path=path,
            text=load_text(path),
            pass_patterns=[
                r"\ball timing requirements (?:were )?met\b",
                r"\btiming requirements (?:were )?met\b",
            ],
            fail_patterns=[
                r"\btiming requirements (?:were )?not met\b",
                r"\ball timing requirements (?:were )?not met\b",
                r"\bsetup requirements (?:were )?not met\b",
            ],
            slack_patterns=[
                r"Worst-?case setup slack(?: is)?\s*[:=]?\s*(-?[0-9.]+)",
                r"Slack \((?:MET|VIOLATED)\)\s*:\s*(-?[0-9.]+)ns",
                r"Setup Slack\s*[:=]\s*(-?[0-9.]+)\s*ns",
            ],
        )
        if result is not None:
            return result

    fail(
        f"Unable to determine Quartus timing status from {', '.join(str(path) for path in candidates)}"
    )


def check_gowin_tang_primer_25k(build_dir: Path) -> str:
    candidates = unique_existing_paths(
        build_dir,
        [
            "riscv_fpga_timing.rpt",
            "riscv_fpga_timing_summary.rpt",
            "project/**/*timing*.rpt",
            "project/**/*timing*.txt",
            "project/**/*summary*.rpt",
            "project/**/*summary*.txt",
        ],
    )
    if not candidates:
        fail(f"No Gowin timing artifacts found under {build_dir}")

    for path in candidates:
        result = evaluate_report(
            tool_name="Gowin",
            path=path,
            text=load_text(path),
            pass_patterns=[
                r"\bresult\s*:\s*pass\b",
                r"\btiming\s+passed\b",
                r"\ball constraints met\b",
            ],
            fail_patterns=[
                r"\bresult\s*:\s*fail\b",
                r"\btiming\s+failed\b",
                r"\btiming violations?\b",
                r"\bsetup violations?\b",
            ],
            slack_patterns=[
                r"Worst-?case setup slack(?: is)?\s*[:=]?\s*(-?[0-9.]+)",
                r"Setup slack\s*[:=]\s*(-?[0-9.]+)\s*ns",
                r"Slack \((?:MET|VIOLATED)\)\s*:\s*(-?[0-9.]+)ns",
            ],
        )
        if result is not None:
            return result

    fail(
        f"Unable to determine Gowin timing status from {', '.join(str(path) for path in candidates)}"
    )


def main() -> None:
    args = parse_args()
    build_dir = Path(args.build_dir) if args.build_dir else SCRIPT_DIR / "build" / args.target
    if not build_dir.exists():
        fail(f"Build directory does not exist: {build_dir}")

    if args.target == "ecp5_icepi_zero":
        message = check_ecp5_icepi_zero(build_dir)
    elif args.target == "artix7_alchitry_au":
        message = check_artix7_alchitry_au(build_dir)
    elif args.target == "cyclonev_analogue_pocket":
        message = check_cyclonev_analogue_pocket(build_dir)
    else:
        message = check_gowin_tang_primer_25k(build_dir)

    sys.stdout.write(message + "\n")


if __name__ == "__main__":
    main()
