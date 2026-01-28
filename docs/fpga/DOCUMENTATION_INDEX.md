# Extension Configuration Documentation Index

This directory contains comprehensive documentation for the RISC-V CPU extension configuration feature.

## Quick Start

**New to extension configuration?** Start here:
1. 📖 [QUICK_REFERENCE.md](QUICK_REFERENCE.md) - 2-minute overview
2. 🎯 [ARCHITECTURE_DIAGRAM.md](ARCHITECTURE_DIAGRAM.md) - Visual architecture guide
3. 📚 [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md) - Detailed usage guide

## Documentation Files

### User-Facing Documentation

| Document | Purpose | Audience |
|----------|---------|----------|
| [QUICK_REFERENCE.md](QUICK_REFERENCE.md) | TL;DR parameter summary and common configs | All users |
| [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md) | Complete configuration guide with examples | FPGA designers |
| [ARCHITECTURE_DIAGRAM.md](ARCHITECTURE_DIAGRAM.md) | Visual architecture and resource allocation | Hardware engineers |
| [TESTING_EXTENSIONS.md](TESTING_EXTENSIONS.md) | Testing and verification procedures | QA/Verification engineers |

### Developer Documentation

| Document | Purpose | Audience |
|----------|---------|----------|
| [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md) | Implementation details and verification | Developers/Maintainers |
| [README.md](README.md) | Main project README (updated with config section) | All users |

## Reading Path by Role

### FPGA Designer
*Goal: Configure CPU for target device*

1. [QUICK_REFERENCE.md](QUICK_REFERENCE.md) - Learn parameters
2. [ARCHITECTURE_DIAGRAM.md](ARCHITECTURE_DIAGRAM.md) - Understand resource impact
3. [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md) - Detailed configuration
4. [TESTING_EXTENSIONS.md](TESTING_EXTENSIONS.md) - Verify design

### Software Developer
*Goal: Understand what instructions are available*

1. [QUICK_REFERENCE.md](QUICK_REFERENCE.md) - See configurations
2. [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md) - Behavioral changes section

### Verification Engineer
*Goal: Test different configurations*

1. [TESTING_EXTENSIONS.md](TESTING_EXTENSIONS.md) - Testing procedures
2. [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md) - Verification checklist

### Project Maintainer
*Goal: Understand implementation*

1. [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md) - Complete overview
2. [ARCHITECTURE_DIAGRAM.md](ARCHITECTURE_DIAGRAM.md) - Design rationale
3. [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md) - Implementation details

## Document Summaries

### QUICK_REFERENCE.md
- **Length:** ~1 page
- **Format:** Quick lookup tables
- **Content:** Parameters, common configs, verification commands
- **Best For:** Quick reference while coding

### EXTENSION_CONFIG.md
- **Length:** ~10 pages
- **Format:** Comprehensive guide
- **Content:** Full parameter docs, usage examples, behavioral changes, synthesis info
- **Best For:** First-time configuration, detailed questions

### ARCHITECTURE_DIAGRAM.md
- **Length:** ~5 pages
- **Format:** ASCII diagrams and tables
- **Content:** Module hierarchy, resource allocation, conditional compilation
- **Best For:** Understanding design structure, planning configurations

### TESTING_EXTENSIONS.md
- **Length:** ~5 pages
- **Format:** Step-by-step procedures
- **Content:** Linting, testing, manual verification
- **Best For:** Verifying configurations work correctly

### IMPLEMENTATION_SUMMARY.md
- **Length:** ~6 pages
- **Format:** Checklist and verification results
- **Content:** Changes made, verification status, success criteria
- **Best For:** Code review, understanding what was implemented

## Common Questions

### "How do I save LUTs for my iCE40-HX8K?"
→ See [QUICK_REFERENCE.md](QUICK_REFERENCE.md) - Minimal RV32I configuration

### "What happens when I disable an extension?"
→ See [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md) - Behavioral Changes section

### "Will this break my existing code?"
→ No! See [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md) - Backward Compatibility

### "How do I test my configuration?"
→ See [TESTING_EXTENSIONS.md](TESTING_EXTENSIONS.md) - Testing procedures

### "How much logic will I save?"
→ See [ARCHITECTURE_DIAGRAM.md](ARCHITECTURE_DIAGRAM.md) - Resource Allocation section

### "How does the generate block work?"
→ See [ARCHITECTURE_DIAGRAM.md](ARCHITECTURE_DIAGRAM.md) - Conditional Compilation Flow

## File Modification Summary

### Modified RTL Files
- `rtl/alu.sv` - Added ENABLE_M_EXT parameter
- `rtl/top.sv` - Added ENABLE_M_EXT and ENABLE_F_EXT parameters
- `rtl/top_with_peripherals.sv` - Added extension parameters

### New Documentation Files
- `QUICK_REFERENCE.md` - Quick lookup guide
- `EXTENSION_CONFIG.md` - Comprehensive configuration guide
- `ARCHITECTURE_DIAGRAM.md` - Visual architecture documentation
- `TESTING_EXTENSIONS.md` - Testing procedures
- `IMPLEMENTATION_SUMMARY.md` - Implementation verification
- `DOCUMENTATION_INDEX.md` - This file

### Updated Documentation
- `README.md` - Added extension configuration section

## Contributing

When modifying extension configuration:
1. Update relevant documentation
2. Run verification: `verilator --lint-only rtl/*.sv rtl/peripherals/*.sv`
3. Run tests: `cargo test --verbose`
4. Update [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md) if adding features

## Support

For questions about:
- **Configuration:** See [EXTENSION_CONFIG.md](EXTENSION_CONFIG.md)
- **Testing:** See [TESTING_EXTENSIONS.md](TESTING_EXTENSIONS.md)
- **Implementation:** See [IMPLEMENTATION_SUMMARY.md](IMPLEMENTATION_SUMMARY.md)
- **Quick Answers:** See [QUICK_REFERENCE.md](QUICK_REFERENCE.md)

---

*Last Updated: 2025 - Extension Configuration Feature*
