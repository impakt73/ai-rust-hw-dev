# Reference Documentation

This directory contains detailed technical reference documents that are actively relevant to the current codebase.

## Contents

### API and Usage Documentation

- **PROGRAMMATIC_LOADING.md** - Comprehensive guide for programmatically loading instructions and data into the CPU simulator without ELF files
- **packet-protocol.md** - Complete binary packet communication protocol specification for CPU-to-host communication

### RV32F Floating-Point Extension

- **RV32F-README.md** - Overview and index of all RV32F implementation documentation
- **RV32F-STATUS.md** - Detailed implementation status, test results, and technical architecture of the RV32F extension
- **rv32f-testing-guide.md** - Practical guide for implementing comprehensive RV32F tests
- **rv32f-upgrade-plan.md** - Complete technical specification and implementation roadmap for RV32F

### Implementation Deep-Dives

- **bss-optimization.md** - Technical analysis of BSS zero-initialization optimization using `.uninit` section
- **bus-device-registration-system.md** - Detailed specification for the handle-based dynamic bus device registration system

## Usage

These documents serve as reference material for:
- Understanding implemented features and their design decisions
- Maintaining and extending the codebase
- Creating tests and verifying behavior
- Debugging issues related to specific subsystems

All documents in this directory describe features that are currently implemented or provide essential context for the existing code.
