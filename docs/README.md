# Documentation Organization

This directory contains all documentation for the RISC-V hardware verification project.

## Directory Structure

- **agents/** - Documentation files specifically designed for AI coding agents. These files are typically referenced by `AGENTS.md` in the repository root and contain detailed guidelines, conventions, and best practices for agents working on the codebase.

- **fpga/** - FPGA-specific documentation, including synthesis, timing, global-buffer, and resource-analysis reports plus reproduction guides that were moved out of implementation directories to keep `rtl/fpga/` and `fpga/` focused on source files and build assets.

- **plans/** - Implementation plans that are either pending execution or partially executed. These are actionable roadmaps created from research documents and are deleted once they've been fully implemented as code.

- **research/** - Research documents compiled during investigation and exploration sessions. These files contain information gathered during research and are used as source material for creating implementation plans. Research documents are deleted once they've been converted into an implementation plan.

## Document Flow

The intended lifecycle for documentation in this repository follows this pattern:

1. **Research Phase**: Information is gathered and documented in `research/`
2. **Planning Phase**: Research documents are synthesized into implementation plans in `plans/` (research docs are deleted at this step)
3. **Implementation Phase**: Plans are executed and turned into code in the project (implementation plans are deleted at this step)

Persistent reference material that should live alongside the codebase long-term (for example FPGA analysis reports and reproduction guides) belongs in a stable subfolder such as `docs/fpga/` rather than the transient `research/` → `plans/` workflow above.
