# GitHub Copilot Instructions

⚠️ **REQUIRED READING:** Before starting any work on this repository, you **MUST** read [AGENTS.md](../AGENTS.md).

The AGENTS.md file contains all essential information for working on this RISC-V hardware verification project, including:

- Custom agent selection guide (which specialized agent to use for your task)
- Project architecture and design decisions
- Build and test procedures
- Coding conventions and standards
- CI/CD requirements and PR readiness checklist
- Debugging methodologies
- Links to detailed sub-documentation

## Quick Reference

**Getting Started:** See [docs/agents/getting-started.md](../docs/agents/getting-started.md)
- Prerequisites (Verilator installation, etc.)
- Quick start commands
- Common issues and solutions

**For Your Specific Task, Consult:**
- Custom agent selection: [AGENTS.md](../AGENTS.md) → Choose the right specialized agent
- Testing work: [docs/agents/testing.md](../docs/agents/testing.md)
- RTL development: [docs/agents/rtl-development.md](../docs/agents/rtl-development.md)
- Rust development: [docs/agents/rust-development.md](../docs/agents/rust-development.md)
- CI/CD and PR readiness: [docs/agents/ci-cd.md](../docs/agents/ci-cd.md)
- Debugging: [docs/agents/debugging.md](../docs/agents/debugging.md)

## Critical Requirements

1. **Read AGENTS.md first** - Contains custom agent selection and project-wide conventions
2. **Use specialized agents** - Delegate to appropriate custom agents (see AGENTS.md)
3. **No CodeQL scans** - Skip security scanning (not needed for hardware verification)
4. **Mandatory code quality for Rust:**
   - Always run `cargo fmt` before committing
   - Always run `cargo clippy -- -D warnings` before committing
   - Zero tolerance for clippy warnings

**→ Start by reading [AGENTS.md](../AGENTS.md) to understand the project and select the right agent for your task.**
