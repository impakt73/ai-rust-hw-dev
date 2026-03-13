#!/usr/bin/env python3

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional


SCRIPT_DIR = Path(__file__).resolve().parent

MIN_PYTHON = (3, 10)

if sys.version_info < MIN_PYTHON:
    version = ".".join(str(part) for part in MIN_PYTHON)
    sys.stderr.write(
        f"ERROR: fpga_design_stats.py requires Python {version} or higher.\n"
    )
    raise SystemExit(1)

TARGET_CONFIGS = {
    "ice40_alchitry_cu": {
        "target_frequency_mhz": 25.0,
        "preferred_clock_patterns": ["pll_clk_global", "pll_clk", "clk"],
        "timing_sources": ["nextpnr.log"],
        "resource_sources": ["nextpnr.log", "yosys.log"],
    },
    "ecp5_icepi_zero": {
        "target_frequency_mhz": 50.0,
        "preferred_clock_patterns": ["clk", "sys_clk"],
        "timing_sources": ["nextpnr.log"],
        "resource_sources": ["nextpnr.log", "yosys.log"],
    },
    "artix7_alchitry_au": {
        "target_frequency_mhz": 100.0,
        "preferred_clock_patterns": ["clk_100mhz", "clk"],
        "timing_sources": ["riscv_fpga_timing.rpt"],
        "resource_sources": ["riscv_fpga_utilization.rpt"],
    },
}

HEADLINE_RESOURCE_ALIASES = {
    "logic": [
        "ICESTORM_LC",
        "TRELLIS_SLICE",
        "TRELLIS_COMB",
        "Slice LUTs",
        "CLB LUTs",
        "SB_LUT4",
    ],
    "registers": [
        "TRELLIS_FF",
        "Slice Registers",
        "CLB Registers",
        "SB_DFF",
    ],
    "bram": [
        "ICESTORM_RAM",
        "SB_RAM40_4K",
        "DP16KD",
        "Block RAM Tile",
        "RAMB36/FIFO*",
        "RAMB18",
    ],
    "dsp": [
        "MULT18X18D",
        "DSPs",
        "DSP48E1s",
    ],
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Generate standardized FPGA resource and timing statistics for a supported target."
        )
    )
    parser.add_argument(
        "--target",
        choices=sorted(TARGET_CONFIGS),
        required=True,
        help="FPGA target to summarize.",
    )
    parser.add_argument(
        "--build",
        action="store_true",
        help="Build the target with `make TARGET=<target> all` before parsing stats.",
    )
    parser.add_argument(
        "--format",
        choices=("text", "json", "markdown"),
        default="text",
        help="Format to print to stdout.",
    )
    parser.add_argument(
        "--build-dir",
        help="Override the build directory. Defaults to rtl/fpga/build/<target>.",
    )
    args = parser.parse_args()
    if args.build and args.build_dir:
        parser.error("--build cannot be combined with --build-dir")
    return args


def run_build(target: str) -> None:
    command = ["make", "-C", str(SCRIPT_DIR), f"TARGET={target}", "all"]
    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        return

    sys.stderr.write(
        f"FPGA build failed while generating stats for target '{target}'.\n"
    )
    if result.stdout:
        sys.stderr.write(result.stdout)
    if result.stderr:
        sys.stderr.write(result.stderr)
    raise SystemExit(result.returncode)


def load_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def parse_nextpnr_clocks(text: str) -> List[Dict[str, Any]]:
    matches: List[Dict[str, Any]] = []
    # Matches nextpnr timing summaries such as:
    # Max frequency for clock 'pll_clk_global': 64.69 MHz (PASS at 25.00 MHz)
    pattern = re.compile(
        r"Max frequency for clock\s+'([^']+)':\s+([0-9.]+)\s+MHz\s+\((PASS|FAIL)\s+at\s+([0-9.]+)\s+MHz\)"
    )
    for match in pattern.finditer(text):
        matches.append(
            {
                "clock_name": match.group(1),
                "max_frequency_mhz": float(match.group(2)),
                "status": match.group(3),
                "target_frequency_mhz": float(match.group(4)),
                "source_file": "nextpnr.log",
            }
        )
    return matches


def parse_vivado_timing(
    text: str, preferred_clock_patterns: List[str]
) -> Optional[Dict[str, Any]]:
    wns = None
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if "WNS(ns)" not in line:
            continue
        for candidate in lines[index + 1 : index + 8]:
            if set(candidate.strip()) <= {"-", " "}:
                continue
            numbers = re.findall(r"-?\d+(?:\.\d+)?", candidate)
            if numbers:
                wns = float(numbers[0])
                break
        if wns is not None:
            break

    clock_rows: List[Dict[str, float]] = []
    # Matches Vivado Clock Summary rows in either table form:
    # | clk_100mhz | {0.000 5.000} | 10.000 | 100.000 |
    # or whitespace-delimited form:
    # clk_100mhz {0.000 5.000} 10.000 100.000
    pipe_pattern = re.compile(
        r"^\|\s*([^|]+?)\s*\|\s*\{[^|]+\}\s*\|\s*([0-9.]+)\s*\|\s*([0-9.]+)\s*\|"
    )
    whitespace_pattern = re.compile(
        r"^\s*(\S+)\s+\{[^}]+\}\s+([0-9.]+)\s+([0-9.]+)\s*$"
    )
    for line in lines:
        pipe_match = pipe_pattern.match(line)
        whitespace_match = whitespace_pattern.match(line)
        match = pipe_match or whitespace_match
        if match is None:
            continue
        clock_rows.append(
            {
                "clock_name": match.group(1).strip(),
                "period_ns": float(match.group(2)),
                "target_frequency_mhz": float(match.group(3)),
            }
        )

    selected_clock = select_clock(
        [
            {
                "clock_name": row["clock_name"],
                "max_frequency_mhz": row["target_frequency_mhz"],
                "target_frequency_mhz": row["target_frequency_mhz"],
                "period_ns": row["period_ns"],
                "status": "UNKNOWN",
                "source_file": "riscv_fpga_timing.rpt",
            }
            for row in clock_rows
        ],
        preferred_clock_patterns,
    )
    if selected_clock is None or wns is None:
        return None

    achieved_period_ns = selected_clock["period_ns"] - wns
    if achieved_period_ns <= 0:
        return None

    max_frequency_mhz = 1000.0 / achieved_period_ns
    return {
        "clock_name": selected_clock["clock_name"],
        "max_frequency_mhz": max_frequency_mhz,
        "target_frequency_mhz": selected_clock["target_frequency_mhz"],
        "status": "PASS" if wns >= 0 else "FAIL",
        "wns_ns": wns,
        "source_file": "riscv_fpga_timing.rpt",
    }


def parse_nextpnr_resources(text: str) -> Dict[str, Dict[str, float]]:
    resources: Dict[str, Dict[str, float]] = {}
    pattern = re.compile(
        r"^\s*(?:Info:\s*)?([A-Za-z0-9_./ +-]+):\s+([0-9]+)\s*/\s*([0-9]+)\s+([0-9.]+)%\s*$",
        re.MULTILINE,
    )
    for match in pattern.finditer(text):
        name = match.group(1).strip()
        resources[name] = {
            "used": int(match.group(2)),
            "available": int(match.group(3)),
            "utilization_percent": float(match.group(4)),
        }
    return resources


def parse_vivado_resources(text: str) -> Dict[str, Dict[str, float]]:
    resources: Dict[str, Dict[str, float]] = {}
    # Matches Vivado utilization rows such as:
    # | Slice LUTs | 4,200 | 20,800 | 20.19 |
    pattern = re.compile(
        r"^\|\s*([^|]+?)\s*\|\s*([0-9,]+)\s*\|\s*([0-9,]+)\s*\|\s*([0-9.]+)\s*\|",
        re.MULTILINE,
    )
    for match in pattern.finditer(text):
        name = match.group(1).strip()
        if not name or set(name) <= {"-", " "}:
            continue
        if name in {"Site Type", "Ref Name", "Primitive"}:
            continue
        resources[name] = {
            "used": int(match.group(2).replace(",", "")),
            "available": int(match.group(3).replace(",", "")),
            "utilization_percent": float(match.group(4)),
        }
    return resources


def parse_yosys_cell_counts(text: str) -> Dict[str, int]:
    blocks = re.findall(
        r"Number of cells:\s+[0-9]+\s*\n((?:\s+\S+\s+[0-9]+\n)+)",
        text,
        flags=re.MULTILINE,
    )
    if not blocks:
        return {}

    cell_counts: Dict[str, int] = {}
    for line in blocks[-1].splitlines():
        match = re.match(r"\s+(\S+)\s+([0-9]+)\s*$", line)
        if match is None:
            continue
        cell_counts[match.group(1)] = int(match.group(2))
    return cell_counts


def select_clock(
    clocks: List[Dict[str, Any]], preferred_clock_patterns: List[str]
) -> Optional[Dict[str, Any]]:
    if not clocks:
        return None

    lowered_patterns = [pattern.lower() for pattern in preferred_clock_patterns]
    for pattern in lowered_patterns:
        for clock in clocks:
            if pattern in clock["clock_name"].lower():
                return clock

    return min(clocks, key=lambda clock: clock["max_frequency_mhz"])


def build_headline_resources(
    post_route_resources: Dict[str, Dict[str, float]],
    synthesis_cell_counts: Dict[str, int],
) -> Dict[str, Dict[str, Any]]:
    headline_resources: Dict[str, Dict[str, Any]] = {}
    for category, aliases in HEADLINE_RESOURCE_ALIASES.items():
        selected_name = None
        selected_value = None
        for alias in aliases:
            if alias in post_route_resources:
                selected_value = {
                    "name": alias,
                    **post_route_resources[alias],
                    "source": "post_route_resources",
                }
                break
        if selected_value is None:
            for alias in aliases:
                if alias in synthesis_cell_counts:
                    selected_value = {
                        "name": alias,
                        "used": synthesis_cell_counts[alias],
                        "source": "synthesis_cell_counts",
                    }
                    break
        if selected_value is not None:
            headline_resources[category] = selected_value
    return headline_resources


def collect_source_metadata(build_dir: Path, relative_paths: List[str]) -> List[Dict[str, Any]]:
    return [
        {
            "path": str(build_dir / relative_path),
            "exists": (build_dir / relative_path).exists(),
        }
        for relative_path in relative_paths
    ]


def collect_stats(target: str, build_dir: Path) -> Dict[str, Any]:
    config = TARGET_CONFIGS[target]

    nextpnr_log = build_dir / "nextpnr.log"
    yosys_log = build_dir / "yosys.log"
    timing_report = build_dir / "riscv_fpga_timing.rpt"
    utilization_report = build_dir / "riscv_fpga_utilization.rpt"

    timing = None
    post_route_resources: Dict[str, Dict[str, float]] = {}
    synthesis_cell_counts: Dict[str, int] = {}
    source_artifacts = {
        "timing": collect_source_metadata(build_dir, config["timing_sources"]),
        "resources": collect_source_metadata(build_dir, config["resource_sources"]),
    }

    if nextpnr_log.exists():
        nextpnr_text = load_text(nextpnr_log)
        timing = select_clock(
            parse_nextpnr_clocks(nextpnr_text),
            config["preferred_clock_patterns"],
        )
        post_route_resources = parse_nextpnr_resources(nextpnr_text)

    if target == "artix7_alchitry_au":
        if timing_report.exists():
            vivado_timing_text = load_text(timing_report)
            timing = parse_vivado_timing(
                vivado_timing_text, config["preferred_clock_patterns"]
            )
        if utilization_report.exists():
            post_route_resources = parse_vivado_resources(load_text(utilization_report))

    if yosys_log.exists():
        synthesis_cell_counts = parse_yosys_cell_counts(load_text(yosys_log))

    headline_resources = build_headline_resources(
        post_route_resources, synthesis_cell_counts
    )

    if timing is None:
        expected_timing = ", ".join(entry["path"] for entry in source_artifacts["timing"])
        raise ValueError(
            f"Unable to parse timing information for target '{target}' from {build_dir}. "
            f"Expected one of: {expected_timing}"
        )
    if not post_route_resources and not synthesis_cell_counts:
        expected_resources = ", ".join(
            entry["path"] for entry in source_artifacts["resources"]
        )
        raise ValueError(
            f"Unable to parse utilization information for target '{target}' from {build_dir}. "
            f"Expected one of: {expected_resources}"
        )

    target_frequency_mhz = timing.get(
        "target_frequency_mhz", config["target_frequency_mhz"]
    )
    max_frequency_mhz = timing["max_frequency_mhz"]

    return {
        "target": target,
        "build_dir": str(build_dir),
        "target_frequency_mhz": target_frequency_mhz,
        "max_frequency_mhz": max_frequency_mhz,
        "timing_status": "PASS" if max_frequency_mhz >= target_frequency_mhz else "FAIL",
        "timing_margin_mhz": max_frequency_mhz - target_frequency_mhz,
        "timing_margin_percent": (
            ((max_frequency_mhz - target_frequency_mhz) / target_frequency_mhz) * 100.0
        ),
        "timing": timing,
        "headline_resources": headline_resources,
        "post_route_resources": post_route_resources,
        "synthesis_cell_counts": synthesis_cell_counts,
        "source_artifacts": source_artifacts,
        "artifacts": {
            "json": str(build_dir / "riscv_fpga_stats.json"),
            "markdown": str(build_dir / "riscv_fpga_stats.md"),
        },
    }


def render_text(stats: Dict[str, Any]) -> str:
    lines = [
        f"Target: {stats['target']}",
        f"Build dir: {stats['build_dir']}",
        (
            "Timing: "
            f"{stats['max_frequency_mhz']:.2f} MHz vs target {stats['target_frequency_mhz']:.2f} MHz "
            f"({stats['timing_status']}, {stats['timing_margin_percent']:+.1f}%)"
        ),
        f"Timing source: {stats['timing']['source_file']} ({stats['timing']['clock_name']})",
    ]

    if stats["headline_resources"]:
        lines.append("Headline resources:")
        for category, resource in stats["headline_resources"].items():
            summary = f"  - {category}: {resource['name']} = {resource['used']}"
            if "available" in resource:
                summary += (
                    f"/{resource['available']} ({resource['utilization_percent']:.2f}%)"
                )
            lines.append(summary)

    if stats["post_route_resources"]:
        lines.append("Post-route resource table:")
        for name in sorted(stats["post_route_resources"]):
            resource = stats["post_route_resources"][name]
            lines.append(
                f"  - {name}: {resource['used']}/{resource['available']} "
                f"({resource['utilization_percent']:.2f}%)"
            )

    if stats["synthesis_cell_counts"]:
        lines.append("Post-synthesis cell counts:")
        for name in sorted(stats["synthesis_cell_counts"]):
            lines.append(f"  - {name}: {stats['synthesis_cell_counts'][name]}")

    lines.append(f"JSON: {stats['artifacts']['json']}")
    lines.append(f"Markdown: {stats['artifacts']['markdown']}")
    return "\n".join(lines)


def render_markdown(stats: Dict[str, Any]) -> str:
    lines = [
        f"# FPGA Design Stats - {stats['target']}",
        "",
        "| Metric | Value |",
        "| --- | --- |",
        f"| Target frequency | {stats['target_frequency_mhz']:.2f} MHz |",
        f"| Max frequency | {stats['max_frequency_mhz']:.2f} MHz |",
        f"| Timing status | {stats['timing_status']} |",
        f"| Timing margin | {stats['timing_margin_mhz']:+.2f} MHz ({stats['timing_margin_percent']:+.1f}%) |",
        f"| Timing source | `{stats['timing']['source_file']}` ({stats['timing']['clock_name']}) |",
        "",
    ]

    if stats["headline_resources"]:
        lines.extend(
            [
                "## Headline Resources",
                "",
                "| Category | Resource | Used | Available | Utilization |",
                "| --- | --- | ---: | ---: | ---: |",
            ]
        )
        for category, resource in stats["headline_resources"].items():
            available = resource.get("available", "")
            utilization = (
                f"{resource['utilization_percent']:.2f}%"
                if "utilization_percent" in resource
                else ""
            )
            lines.append(
                f"| {category} | {resource['name']} | {resource['used']} | {available} | {utilization} |"
            )
        lines.append("")

    if stats["post_route_resources"]:
        lines.extend(
            [
                "## Post-Route Resource Utilization",
                "",
                "| Resource | Used | Available | Utilization |",
                "| --- | ---: | ---: | ---: |",
            ]
        )
        for name in sorted(stats["post_route_resources"]):
            resource = stats["post_route_resources"][name]
            lines.append(
                f"| {name} | {resource['used']} | {resource['available']} | {resource['utilization_percent']:.2f}% |"
            )
        lines.append("")

    if stats["synthesis_cell_counts"]:
        lines.extend(
            [
                "## Post-Synthesis Cell Counts",
                "",
                "| Cell | Count |",
                "| --- | ---: |",
            ]
        )
        for name in sorted(stats["synthesis_cell_counts"]):
            lines.append(f"| {name} | {stats['synthesis_cell_counts'][name]} |")
        lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def main() -> None:
    args = parse_args()
    build_dir = Path(args.build_dir) if args.build_dir else SCRIPT_DIR / "build" / args.target

    if args.build:
        run_build(args.target)
    elif not build_dir.exists():
        sys.stderr.write(f"ERROR: Build directory does not exist: {build_dir}\n")
        raise SystemExit(1)

    stats = collect_stats(args.target, build_dir)
    json_output = build_dir / "riscv_fpga_stats.json"
    markdown_output = build_dir / "riscv_fpga_stats.md"
    json_output.write_text(json.dumps(stats, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    markdown_output.write_text(render_markdown(stats), encoding="utf-8")

    if args.format == "json":
        sys.stdout.write(json.dumps(stats, indent=2, sort_keys=True) + "\n")
    elif args.format == "markdown":
        sys.stdout.write(render_markdown(stats))
    else:
        sys.stdout.write(render_text(stats) + "\n")


if __name__ == "__main__":
    main()
