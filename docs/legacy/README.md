# Legacy Documentation

This directory contains historical implementation plans and proposals that are no longer actively used but are kept for reference and historical context.

## Contents

### Implemented Features (Historical Plans)

- **rv32a-atomic-extension-plan.md** - Original implementation plan for RV32A atomic extension (now implemented)
- **rv32c-implementation-plan.md** - Original implementation plan for RV32C compressed instruction extension (now implemented)

### Status Reports and Summaries

- **packet-protocol-implementation.md** - Historical status summary of packet protocol implementation work

### Proposed Features (Not Yet Implemented)

- **synthesizable-division-unit-plan.md** - Plan for replacing non-synthesizable division with hardware-friendly algorithm
- **video-audio-output-plan.md** - Plan for adding real-time video and audio output capabilities to the simulator
- **mcp-server-api-improvements.md** - Recommendations for VCD/MCP server API enhancements

## Why These Documents Are Here

These documents are preserved in the legacy directory because:

1. **Historical Context** - They document the thought process and design decisions that went into implemented features
2. **Future Reference** - Implementation plans for not-yet-implemented features may be useful when those features are eventually added
3. **Learning Resource** - They demonstrate the planning and design methodology used in this project
4. **Completeness** - Maintaining a complete historical record of the project's evolution

## Using Legacy Documents

When referencing these documents:
- ✅ Use them to understand why certain design decisions were made
- ✅ Use them as templates for creating new implementation plans
- ⚠️ Be aware that implemented features may have evolved beyond what's described in these plans
- ⚠️ Check the current codebase for the actual implementation details
- ⚠️ For proposals not yet implemented, treat them as ideas rather than specifications

For current technical documentation, see the `docs/reference/` directory instead.
