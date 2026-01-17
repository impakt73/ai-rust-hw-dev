# Devcontainer Configuration

This directory contains the development container configuration for the RISC-V RV32IM CPU verification project.

## Overview

The devcontainer provides a fully configured development environment with all dependencies pre-installed, ensuring consistent development across different machines and platforms.

## What's Included

### System Packages
- **Verilator** - SystemVerilog simulator (critical for this project)
- **gcc-riscv64-unknown-elf** - RISC-V GCC cross-compiler toolchain
- **Build tools** - Standard development tools (gcc, make, git, etc.)

### Rust Toolchain
- **Rust stable** - Latest stable Rust compiler
- **riscv32imafc-unknown-none-elf** - RISC-V 32-bit target for cross-compilation
- **cargo-watch** - Watch for changes and auto-rebuild
- **cargo-expand** - Expand macros for debugging

### VS Code Extensions
- **Rust Analyzer** - Advanced Rust language support
- **SystemVerilog** - Syntax highlighting and IntelliSense for .sv files
- **GitHub Copilot** - AI-powered code suggestions
- **GitLens** - Enhanced git integration

## Usage

### Using with VS Code

1. Install the "Dev Containers" extension in VS Code
2. Open the repository in VS Code
3. Click "Reopen in Container" when prompted (or use Command Palette: "Dev Containers: Reopen in Container")
4. Wait for the container to build (first time only, ~5-10 minutes)
5. Start coding!

### Using with GitHub Codespaces

1. Navigate to the repository on GitHub
2. Click the "Code" button → "Codespaces" tab → "Create codespace on main"
3. Wait for the environment to initialize
4. Start coding in the browser-based VS Code!

### Manual Docker Usage

```bash
# Build the container
docker build -t riscv-dev .devcontainer

# Run the container
docker run -it -v $(pwd):/workspace riscv-dev
```

## Quick Start Commands

Once inside the devcontainer:

```bash
# Run all tests
cargo test

# Build the project
cargo build

# Format code
cargo fmt

# Lint Rust code
cargo clippy -- -D warnings

# Lint SystemVerilog
verilator --lint-only rtl/*.sv

# Clean build artifacts (needed after RTL changes)
cargo clean
```

## Dependencies Synchronized With

This devcontainer configuration is based on:
- `.github/workflows/copilot-setup-steps.yml` - GitHub Copilot setup requirements
- `.github/workflows/ci.yml` - CI/CD pipeline dependencies
- `.github/copilot-instructions.md` - Development prerequisites

## Troubleshooting

### Container build fails
- Ensure Docker is running and has sufficient resources (at least 4GB RAM)
- Check your internet connection (packages are downloaded during build)

### Tests fail in container
- Run `cargo clean` to clear cached Verilator builds
- Verify Verilator is installed: `verilator --version`

### VS Code extensions not working
- Reload the window: Command Palette → "Developer: Reload Window"
- Rebuild the container: Command Palette → "Dev Containers: Rebuild Container"

## Benefits

✅ **Consistency** - Same environment for all developers  
✅ **Zero setup** - No manual dependency installation  
✅ **Isolation** - Doesn't affect host system  
✅ **CI/CD alignment** - Matches GitHub Actions environment  
✅ **Cross-platform** - Works on Windows, Mac, and Linux  

## Additional Resources

- [Dev Containers Documentation](https://code.visualstudio.com/docs/devcontainers/containers)
- [GitHub Codespaces Documentation](https://docs.github.com/en/codespaces)
- Project documentation: `AGENTS.md`
