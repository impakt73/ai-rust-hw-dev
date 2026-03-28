# Custom GitHub Runner Image — Research & Migration Guide

## Summary

This document records the investigation into building a pre-baked custom runner
image that eliminates per-job tool installation in both the CI workflow and the
Copilot setup-steps workflow.  It covers the analysis of the current state,
the chosen approach, caveats, and the exact file changes needed to activate the
image once it has been built and published.

---

## Problem Statement

Every CI job and every Copilot coding-agent session currently spends 2–5 minutes
installing the same heavy apt packages (Verilator, yosys, nextpnr-ecp5, FPGA
trellis libraries, openFPGALoader) and setting up the Rust toolchain, even though
those tools never change between runs.

`awalsh128/cache-apt-pkgs-action` mitigates this on cache hits, but:

- Cache misses (new cache key, first run of a PR, weekly cache expiry) pay the
  full install cost.
- The cache does not help with Rust toolchain setup
  (`actions-rust-lang/setup-rust-toolchain@v1`), which always downloads the
  manifest and verifies the toolchain.
- The Copilot agent's `copilot-setup-steps` job runs on every agent session start
  — a slow setup step directly delays the agent's first useful action.

Additionally, the two workflow files have quietly diverged in their package lists:

| Package | `ci.yml` | `copilot-setup-steps.yml` |
|---|---|---|
| `verilator` | ✅ | ✅ |
| `yosys` | ✅ | ✅ |
| `fpga-trellis` | ✅ | ✅ |
| `fpga-trellis-database` | ✅ | ✅ |
| `nextpnr-ecp5` | ✅ | ✅ |
| `openfpgaloader` | ✅ | ✅ |
| `libasound2-dev` | ✅ | ✅ |
| `libudev-dev` | ✅ | ✅ |
| `gcc-riscv64-unknown-elf` | ❌ | ✅ |

The devcontainer also has its own separate list (no FPGA tools; Verilator from
source).  There is no single source of truth.

---

## Chosen Approach: Docker Container Job (`container:` key)

### Why not GitHub Larger Runners with custom images?

GitHub's "custom AMI/image" feature for hosted runners requires the **Team or
Enterprise plan**.  The `container:` key works on **all GitHub plans** (Free,
Pro, Team, Enterprise) and does not require self-hosted runners.

### How it works

```yaml
jobs:
  test:
    runs-on: ubuntu-latest          # Standard GitHub-hosted runner
    container:
      image: ghcr.io/impakt73/ai-rust-hw-dev/runner:latest
      credentials:
        username: ${{ github.actor }}
        password: ${{ secrets.GITHUB_TOKEN }}
```

GitHub spins up an `ubuntu-latest` host, then runs every step inside the
specified container. This trades per-job package installation for a GHCR image
pull, making startup time more predictable and moving tool setup work into a
separate image-build workflow.

### Trade-offs

| | Current (`cache-apt-pkgs-action`) | Custom image (`container:`) |
|---|---|---|
| Startup bottleneck | apt + rustup setup on each job | GHCR image pull on each job |
| Package list drift | Two files, easy to forget | One Dockerfile |
| GitHub plan required | Free | Free |
| Image maintenance | None | Must rebuild on Ubuntu updates |
| Cargo deps cached | Yes (`actions/cache`) | Yes, with cache-path update |

---

## Verilator Version Caveat

The devcontainer builds Verilator **from source** with the comment:

> "to ensure compatibility with marlin / the `--lib-create` and `-j 0` options
> used by the test harness"

The CI workflow uses the **apt package** (`ubuntu-24.04` ships Verilator 5.x).
CI tests currently pass with the apt version, so the runner image also uses the
apt version to match CI behaviour.

**Action required before activating the custom image:** Verify that
`verilator --version` in CI matches what the devcontainer builds from source.
If the apt version is too old (e.g. missing `--lib-create`), the runner
Dockerfile must be updated to build from source — see `.devcontainer/Dockerfile`
for the build recipe.

---

## New Files Created

### `.github/runner/Dockerfile`

The unified package manifest and Rust toolchain definition.  This is the
single source of truth for all tools required by CI and Copilot sessions.

Key design decisions:
- `ubuntu:24.04` base — mirrors the OS version used by GitHub's `ubuntu-latest`
  hosted runners (GitHub switched `ubuntu-latest` to 24.04 in mid-2024).
  Keeping the base version in sync with the host runner avoids subtle glibc /
  kernel ABI differences between the container and its host.
- Rust installed system-wide (`RUSTUP_HOME=/usr/local/rustup`,
  `CARGO_HOME=/usr/local/cargo`) so that container jobs running as root find
  `cargo`/`rustc` on `$PATH` without sourcing `.cargo/env`.
- `riscv32imafc-unknown-none-elf` target baked in.
- `gcc-riscv64-unknown-elf` included (closes the drift vs. `copilot-setup-steps.yml`).
- End-to-end smoke test at build time — a broken install fails the image build,
  not a CI run.

### `.github/workflows/build-runner-image.yml`

Builds and pushes `ghcr.io/impakt73/ai-rust-hw-dev/runner:latest` to GHCR whenever
`.github/runner/Dockerfile` changes on `main`, or on manual dispatch.

Uses Docker Buildx with GHA layer caching so incremental rebuilds (e.g. bumping
a single package version) are fast.

---

## Migration Steps (activate after image is built and verified)

### Step 1 — Build and verify the image locally

```bash
# Build — context is the .github/runner directory (same as the workflow).
# Pass your actual repo URL so the GHCR label is correct.
docker build \
  --build-arg REPO_URL=https://github.com/impakt73/ai-rust-hw-dev \
  -t ghcr.io/impakt73/ai-rust-hw-dev/runner:test \
  .github/runner

# Smoke-test interactively
docker run --rm ghcr.io/impakt73/ai-rust-hw-dev/runner:test bash -c "
  verilator --version
  yosys --version
  nextpnr-ecp5 --version
  riscv64-unknown-elf-gcc --version | head -1
  rustc --version
  cargo --version
  rustup target list --installed | grep riscv32imafc
"
```

### Step 2 — Push the image via `build-runner-image.yml`

Merge a change to `.github/runner/Dockerfile` into `main` (or trigger
`workflow_dispatch`) to publish `ghcr.io/impakt73/ai-rust-hw-dev/runner:latest`.

Make the GHCR package **public** (repository Settings → Packages → runner →
Change visibility) *or* ensure workflows pass `GITHUB_TOKEN` credentials to
pull private packages.

### Step 3 — Update `ci.yml`

```diff
 jobs:
   test:
     runs-on: ubuntu-latest
+    container:
+      image: ghcr.io/impakt73/ai-rust-hw-dev/runner:latest
+      credentials:
+        username: ${{ github.actor }}
+        password: ${{ secrets.GITHUB_TOKEN }}
     env:
       FPGA_TOOL_CACHE_VERSION: v4

     steps:
     - uses: actions/checkout@v4

-    - name: Cache and install system packages
-      uses: awalsh128/cache-apt-pkgs-action@v1
-      with:
-        packages: verilator libasound2-dev libudev-dev yosys fpga-trellis fpga-trellis-database nextpnr-ecp5 openfpgaloader
-        version: v4
-
-    - name: Verify Verilator installation
-      run: verilator --version

     - name: Run RTL lint
       run: find rtl/common -name '*.sv' -exec verilator --lint-only -Wno-MULTITOP {} +

-    - name: Install Rust
-      uses: actions-rust-lang/setup-rust-toolchain@v1
-      with:
-        toolchain: stable
-        cache: false
-
-    - name: Install RISC-V Rust Target
-      run: rustup target add riscv32imafc-unknown-none-elf
-
     - name: Verify FPGA tools installation
       run: |
         yosys --version
         nextpnr-ecp5 --version
```

The `actions/cache@v4` steps for Cargo and FPGA synthesis artifacts are
**mostly kept as-is** — Cargo dependencies are project-specific and not baked
into the image, but the Cargo cache paths must be updated from `~/.cargo/...`
to `/usr/local/cargo/...` (or the workflow must export `CARGO_HOME`) because
the runner image installs Rust system-wide.

If the GHCR package remains private and this workflow later adds an explicit
`permissions:` block, include `packages: read` so the container pull can
authenticate with `GITHUB_TOKEN`.

### Step 4 — Update `copilot-setup-steps.yml`

Apply the same diff pattern as Step 3.  The `container:` key tells the Copilot
coding agent what environment it is running in, replacing the install steps
with an image pull.

```diff
 jobs:
   copilot-setup-steps:
     runs-on: ubuntu-latest
+    container:
+      image: ghcr.io/impakt73/ai-rust-hw-dev/runner:latest
+      credentials:
+        username: ${{ github.actor }}
+        password: ${{ secrets.GITHUB_TOKEN }}
      permissions:
        contents: read
        packages: read

     steps:
       - name: Checkout repository
         uses: actions/checkout@v4

-      - name: Cache and install apt packages
-        uses: awalsh128/cache-apt-pkgs-action@v1
-        with:
-          packages: verilator gcc-riscv64-unknown-elf libasound2-dev libudev-dev yosys fpga-trellis fpga-trellis-database nextpnr-ecp5 openfpgaloader
-          version: v4
-
-      - name: Install Rust Toolchain
-        uses: actions-rust-lang/setup-rust-toolchain@v1
-        with:
-          toolchain: stable
-          cache: false
-
-      - name: Install RISC-V Rust Target
-        run: rustup target add riscv32imafc-unknown-none-elf
-
```

### Step 5 — Verify on a PR

Open a PR with the Step 3 + 4 changes.  Confirm both workflow jobs pass and
note the wall-clock improvement in the job summary.

---

## GitHub Plan & Runner Requirements

| Requirement | Satisfied by |
|---|---|
| `container:` key support | All GitHub Plans (Free+) |
| GHCR private package pull | `GITHUB_TOKEN` with `packages: read` |
| Docker available on runner | Yes — all `ubuntu-*` hosted runners have Docker |
| Self-hosted runner | Not required |
| Larger Runner subscription | Not required |

The only plan-gated feature not used here is "custom AMI images" for hosted
runners (Team/Enterprise).  The `container:` approach is the universally
compatible alternative.

---

## Future Improvements

- **Pin image by digest** in workflow files instead of `:latest` for full
  reproducibility (`image: ghcr.io/impakt73/ai-rust-hw-dev/runner@sha256:...`).
- **Nightly rebuild workflow** — schedule `build-runner-image.yml` weekly to
  pick up Ubuntu security patches without waiting for a Dockerfile edit.
- **Multi-platform image** — add `platforms: linux/amd64,linux/arm64` to
  `build-push-action` if ARM-based runners are ever needed.
- **Devcontainer unification** — consider rebasing `.devcontainer/Dockerfile`
  on `.github/runner/Dockerfile` (or a shared base layer) and adding the FPGA
  tools that are currently missing from the devcontainer.
