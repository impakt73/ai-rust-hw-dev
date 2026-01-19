# Documentation Directory Structure

This directory contains all project documentation organized into several subdirectories based on purpose and relevance.

## Directory Organization

### `/docs/agents/` ✅ Clean and up-to-date
Agent-specific instructions for GitHub Copilot and custom AI coding agents. These files provide context and guidelines for AI-assisted development.

**Files:**
- Getting started guide
- RTL development instructions
- Rust development instructions
- Testing procedures
- Debugging workflows
- CI/CD automation

### `/docs/plans/` ✅ Clean and up-to-date
Active implementation plans for features currently being developed or planned for the near future.

### `/docs/multi-cycle-implementation/` ✅ Clean and up-to-date
Detailed documentation of the multi-cycle CPU architecture implementation, organized as a step-by-step guide.

### `/docs/reference/` 📚 Technical Reference Documentation
**Purpose:** Contains detailed technical reference documents that are actively relevant to the current codebase.

**Contents:**
- API and usage guides (programmatic loading, packet protocol)
- RV32F floating-point extension comprehensive documentation
- Implementation deep-dives (BSS optimization, bus device system)

**See:** [reference/README.md](reference/README.md) for detailed index

### `/docs/legacy/` 📦 Historical Documentation
**Purpose:** Preserves historical implementation plans and proposals that are no longer actively used but provide valuable context.

**Contents:**
- Historical implementation plans for completed features (RV32A, RV32C)
- Proposals for future features (division unit, video/audio output)
- Status summaries and recommendations

**See:** [legacy/README.md](legacy/README.md) for detailed index

## Document Categorization

### When to add a document to each directory:

- **`/agents/`** - Agent-specific instructions and guidelines for AI-assisted development
- **`/plans/`** - Active plans for features in development or immediate future work
- **`/multi-cycle-implementation/`** - Only for multi-cycle architecture documentation (already complete)
- **`/reference/`** - Technical documentation for implemented features and current APIs
- **`/legacy/`** - Historical plans, proposals not yet implemented, or outdated documentation with historical value

## Recently Cleaned Up (2026-01-19)

**Deleted (3 files):**
- `multi-cycle-cpu-implementation-plan.md` - Single large plan file replaced by structured multi-cycle-implementation/ directory
- `end-to-end-test-status.md` - Temporary status report from packet protocol work
- `rv32im-upgrade-plan.md` - Old plan for single-cycle CPU (feature implemented in multi-cycle architecture)

**Moved to reference/ (8 files):**
- Active technical documentation for implemented features

**Moved to legacy/ (6 files):**
- Historical plans and proposals for future reference

## Finding Documentation

1. **For current features:** Check `/docs/reference/`
2. **For AI agent instructions:** Check `/docs/agents/`
3. **For active development:** Check `/docs/plans/`
4. **For historical context:** Check `/docs/legacy/`
5. **For multi-cycle architecture:** Check `/docs/multi-cycle-implementation/`

Each subdirectory contains its own README.md with a detailed index of contents.
