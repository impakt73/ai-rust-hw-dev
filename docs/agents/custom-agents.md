# Custom Agent Selection Guide

This project has **three specialized GitHub Copilot custom agents** to help with different types of work. Choosing the right agent ensures you get the most specialized expertise for your task.

## Quick Decision Tree

```
What do you need to modify?
│
├─ Only .sv files (RTL)?
│  └─ Use: FPGA Architect
│
├─ Only .rs files (Rust)?
│  └─ Use: Rust Verification Architect
│
├─ Both .sv and .rs files?
│  └─ Use: Hardware-Software Integration Architect
│
└─ Agent instructions or documentation?
   └─ Use: AI Instruction Architect
```

## 1. Hardware-Software Integration Architect

**Agent File:** `.github/agents/hw-sw-integration-architect.md`

### When to Use

Use this agent for **cross-layer tasks** that span both hardware and software:

- ✅ Adding or modifying RISC-V instructions (requires both RTL and verification changes)
- ✅ Changing memory interface protocols or handshaking logic
- ✅ Debugging integration issues between RTL and Rust testbench
- ✅ Adding new CPU features (CSRs, instruction extensions, debug capabilities)
- ✅ Performance tuning affecting both hardware and test harnesses
- ✅ Any task explicitly involving both `.sv` and `.rs` files

### Expertise

- Full-stack understanding of RISC-V CPU design AND verification
- Multi-cycle FSM architecture and timing analysis
- Rust-Verilator integration via marlin framework
- Cross-domain debugging and validation

### Example Tasks

- "Add support for the MULH instruction (requires RTL changes and new tests)"
- "Implement memory-mapped I/O for a new peripheral"
- "Debug why load instructions are taking too many cycles"
- "Add instruction trace capability to the CPU"

## 2. FPGA Architect

**Agent File:** `.github/agents/fpga-architect.md`

### When to Use

Use this agent for **pure RTL/hardware design tasks**:

- ✅ Pure RTL refactoring without interface changes
- ✅ Timing optimization and synthesis-focused work
- ✅ Adding internal RTL modules that don't change external interfaces
- ✅ FPGA-specific concerns (resource utilization, clock constraints)
- ✅ State machine design and optimization
- ✅ Clock domain crossing solutions

### Expertise

- Deep SystemVerilog and digital design expertise
- FPGA synthesis and timing closure
- Modern RTL coding standards (IEEE 1800)
- Hardware debugging and optimization

### Example Tasks

- "Optimize the ALU for better timing on FPGAs"
- "Refactor the FSM to reduce combinational depth"
- "Add pipeline registers to improve Fmax"
- "Fix Verilator lint warnings in the decoder module"

## 3. Rust Verification Architect

**Agent File:** `.github/agents/rust-verification-architect.md`

### When to Use

Use this agent for **pure verification/testing tasks**:

- ✅ Adding new test cases without RTL changes
- ✅ Refactoring test infrastructure or test utilities
- ✅ Improving verification methodology
- ✅ Performance testing and benchmarking
- ✅ Debug protocol extensions (FIFO packets, print macros)
- ✅ Rust code quality improvements

### Expertise

- Rust best practices and type system mastery
- Hardware verification patterns and methodologies
- Property-based testing strategies
- FFI and unsafe Rust for Verilator integration

### Example Tasks

- "Add comprehensive tests for all branch instructions"
- "Refactor test helpers to reduce code duplication"
- "Implement property-based testing for the ALU"
- "Add VCD waveform dumping to test infrastructure"

## 4. AI Instruction Architect

**Agent File:** `.github/agents/ai-instruction-architect.md`

### When to Use

Use this agent for **documentation and agent configuration tasks**:

- ✅ Creating or modifying custom agent definitions
- ✅ Updating repository-wide instructions
- ✅ Writing technical documentation for AI agents
- ✅ Improving prompt engineering and agent behavior
- ✅ Reorganizing documentation structure

### Expertise

- Prompt engineering for code generation
- GitHub Copilot configuration best practices
- Documentation structure and progressive disclosure
- Agent instruction design patterns

### Example Tasks

- "Create a new custom agent for debugging tasks"
- "Update the AGENTS.md file with new conventions"
- "Improve agent instructions to avoid common mistakes"
- "Reorganize documentation for better discoverability"

## Important Agent Rules

### For ALL Hardware-Related Agents (FPGA Architect, HW-SW Integration)

**Debugging Methodology:**
- ❌ **Never rely on abstract reasoning** during hardware debugging sessions
- ✅ **Always use concrete data** from simulation via `$display()` statements
- ✅ **Observe actual signal values** before forming hypotheses about hardware behavior
- ✅ **Treat debugging like experimental science:** gather data first, then reason based on evidence

### For ALL Rust-Based Agents (Integration + Verification)

**Memory Management:**
- ❌ Never use `Box::leak()` to circumvent lifetime issues
- ✅ Use callbacks or proper ownership patterns instead
- ✅ The best solution depends on the situation (see Rust Development Guide)

**Code Quality (Mandatory):**
- ✅ Always run `cargo fmt` before committing
- ✅ Always run `cargo clippy -- -D warnings` before committing
- ✅ Address all clippy warnings (zero tolerance)

## Decision Guide Details

### Step 1: Identify What You Need to Modify

Ask yourself:
- "Will I need to change SystemVerilog files?"
- "Will I need to change Rust test files?"
- "Will I need to change both?"

### Step 2: Choose Based on Scope

- **Only `.sv` files** → Use **FPGA Architect**
- **Only `.rs` files** → Use **Rust Verification Architect**
- **Both `.sv` and `.rs` files** → Use **Hardware-Software Integration Architect**
- **Documentation/agent files** → Use **AI Instruction Architect**

### Step 3: Don't Worry About Wrong Choices

- The agents can delegate to each other if needed
- If unsure, default to **Hardware-Software Integration Architect** for safety
- Agents will redirect you if a different specialist is more appropriate

## Delegation Between Agents

Agents are designed to work together:

- **FPGA Architect** can request help from **HW-SW Integration** if test changes are needed
- **Rust Verification Architect** can request help from **HW-SW Integration** if RTL bugs are found
- **HW-SW Integration** can delegate pure RTL work to **FPGA Architect** or pure test work to **Rust Verification Architect**

This ensures you always get the most specialized expertise for each part of your task.

## Best Practices for Using Custom Agents

### 1. Be Specific in Your Request

Good: "Add support for the MULHU instruction including RTL implementation and tests"
Bad: "Make the CPU better"

### 2. Provide Context

Mention:
- What you're trying to achieve
- What you've already tried
- Any relevant error messages or observations

### 3. Trust the Agent's Expertise

- The agents have specialized knowledge and tools
- Follow their recommendations for implementation approach
- They'll ask for clarification if they need more information

### 4. Verify Results

After the agent completes work:
- Review the changes made
- Run tests to verify functionality
- Check that code quality standards are met

## Common Usage Patterns

### Adding a New Instruction

Use: **Hardware-Software Integration Architect**
Why: Requires both RTL decoder changes and comprehensive test coverage

### Fixing a Verilator Warning

Use: **FPGA Architect**
Why: Pure RTL syntax/style issue, no functional changes needed

### Adding Test Coverage

Use: **Rust Verification Architect**
Why: Pure test code, no RTL changes needed

### Debugging Test Failures

Use: **Hardware-Software Integration Architect** (if both RTL and tests might be wrong)
Or: **Rust Verification Architect** (if you're confident RTL is correct)
Or: **FPGA Architect** (if you're confident tests are correct)

### Improving Documentation

Use: **AI Instruction Architect**
Why: Specialized in documentation structure and agent instructions

## Summary

Choose the agent that best matches your task scope:
- **Integration work** → Hardware-Software Integration Architect
- **Pure RTL** → FPGA Architect
- **Pure tests** → Rust Verification Architect
- **Documentation** → AI Instruction Architect

When in doubt, start with the Integration Architect—they can delegate if needed.
