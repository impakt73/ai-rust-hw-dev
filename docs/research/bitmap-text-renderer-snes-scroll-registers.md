# Adding SNES-Style X/Y Scroll Registers to `bitmap_text_renderer`

**Research Document**  
**Context:** Investigate how to add SNES-style horizontal and vertical scroll control to the RTL bitmap text renderer  
**Date:** 2026-03-24

## Executive Summary

The existing `bitmap_text_renderer` is already a tightly timed video pipeline: `video_sync` generates raster coordinates, a character-map ROM lookup selects a glyph index, a font ROM lookup selects an 8-bit row, and the module then delays the full timing bundle one cycle so `pixel_on` stays aligned with the visible coordinates and sync signals (`rtl/common/primitives/bitmap_text_renderer.sv:2-19`, `rtl/common/primitives/bitmap_text_renderer.sv:131-172`, `rtl/common/primitives/bitmap_text_renderer.sv:275-312`).

Adding an SNES-style X/Y scroll register is feasible, but the work is not symmetric:

- **Vertical scroll** is mostly an address-generation change. The renderer already derives tile row and glyph-row index from `sync_active_y`, so replacing those calculations with wrapped scrolled-Y coordinates is straightforward (`rtl/common/primitives/bitmap_text_renderer.sv:200-210`, `rtl/common/primitives/bitmap_text_renderer.sv:242-251`).
- **Horizontal scroll** is substantially harder because the renderer currently assumes the first visible pixel of each line is pixel 0 of tile 0. That assumption is baked into the startup rescue fetch, the blanking-time next-line prefetch, and the line-start/current-tile handoff (`rtl/common/primitives/bitmap_text_renderer.sv:217-252`, `rtl/common/primitives/bitmap_text_renderer.sv:321-323`, `rtl/common/primitives/bitmap_text_renderer.sv:355-372`).

The most robust implementation strategy is:

1. Keep exported `active_x`/`active_y` as **screen-space** coordinates.
2. Introduce separate internal **source-space** coordinates computed from pixel scroll registers.
3. Explicitly wrap by map width/height rather than relying on bit truncation.
4. Latch scroll values at a stable boundary, ideally **frame start** for the first version.
5. Rework first-tile-of-line priming so line start can begin in the middle of a source tile when `scroll_x[2:0] != 0`.

## Why “SNES-Style” Matters Here

For this renderer, “SNES-style scroll” should mean:

- independent `scroll_x` and `scroll_y`
- scroll units measured in **pixels**, not tiles
- low 3 bits select the pixel offset inside an 8x8 tile
- upper bits select the tile column/row
- scrolling wraps around the background map instead of clamping

That matches how a tiled background is normally addressed on classic consoles, and it is the natural fit for this module because the renderer already uses an 8x8 tile map and an 8x8 font ROM (`rtl/common/primitives/bitmap_text_renderer.sv:21-35`, `rtl/common/primitives/bitmap_text_renderer.sv:49-68`).

## Current Renderer Architecture

### Pipeline structure

The module contains four major stages:

1. **Raster timing generation** via `video_sync`, producing `sync_video_de`, `sync_line_start`, `sync_frame_start`, `sync_active_x`, and `sync_active_y` (`rtl/common/primitives/bitmap_text_renderer.sv:74-80`, `rtl/common/primitives/bitmap_text_renderer.sv:131-152`; `rtl/common/primitives/video_sync.sv:29-50`, `rtl/common/primitives/video_sync.sv:111-165`).
2. **Character-map ROM lookup** using `char_map_addr` and `char_map_rdata` (`rtl/common/primitives/bitmap_text_renderer.sv:82-85`, `rtl/common/primitives/bitmap_text_renderer.sv:154-162`).
3. **Font ROM lookup** using `font_addr` and `font_glyph_rdata` (`rtl/common/primitives/bitmap_text_renderer.sv:84-85`, `rtl/common/primitives/bitmap_text_renderer.sv:164-172`).
4. **Buffered tile-row / registered output alignment**, including `current_tile_row_bits`, `next_tile_row_bits`, `pixel_on_next`, and the final output register stage (`rtl/common/primitives/bitmap_text_renderer.sv:89-110`, `rtl/common/primitives/bitmap_text_renderer.sv:254-272`, `rtl/common/primitives/bitmap_text_renderer.sv:275-372`).

### Timing budget

The renderer assumes two synchronous ROMs with two-cycle read latency each:

- `sync_sprom` documents that read data is available **two clock cycles** after the address is presented (`rtl/common/memory/sync_sprom.sv:2-13`, `rtl/common/memory/sync_sprom.sv:55-63`).
- `bitmap_text_renderer` encodes that assumption in `FONT_PIPELINE_CYCLES = 4` and rejects configurations where the horizontal front porch is shorter than that (`rtl/common/primitives/bitmap_text_renderer.sv:53-60`, `rtl/common/primitives/bitmap_text_renderer.sv:194-196`).

This means every change to fetch timing or tile prefetch behavior must preserve the “two ROMs, four cycles total” blanking budget.

### Addressing model today

The visible map is derived directly from the active resolution:

- `TILE_COLUMNS = ACTIVE_WIDTH / TILE_WIDTH`
- `TILE_ROWS = ACTIVE_HEIGHT / TILE_HEIGHT`
- `CHARMAP_DEPTH = TILE_COLUMNS * TILE_ROWS`

(`rtl/common/primitives/bitmap_text_renderer.sv:63-68`)

That means the current background map is exactly screen-sized. It can wrap within the visible area, but there is no notion of a larger off-screen background yet.

## Existing Prefetch Model

The renderer uses three fetch cases:

1. **Startup rescue fetch**: if the first visible line arrives before the next-line prefetch machinery has primed tile 0, issue a request on the first active pixel (`rtl/common/primitives/bitmap_text_renderer.sv:217-229`).
2. **Mid-line next-tile prefetch**: when `active_x_in_tile == 0` and the current tile is not the last tile in the row, prefetch the next tile row (`rtl/common/primitives/bitmap_text_renderer.sv:230-241`).
3. **First-blanking-cycle next-line prefetch**: when active video falls, fetch tile 0 of the next line during horizontal blanking (`rtl/common/primitives/bitmap_text_renderer.sv:242-251`).

Returned font rows are stored in either:

- `current_tile_row_bits` for the tile currently being scanned, or
- `next_tile_row_bits` for the upcoming tile

(`rtl/common/primitives/bitmap_text_renderer.sv:350-372`)

At line start, the module invalidates `current_tile_valid` (`rtl/common/primitives/bitmap_text_renderer.sv:321-323`), which is safe only because the existing design expects the next valid row transition to happen exactly at the beginning of tile 0.

## Where Scroll Must Enter the Design

The renderer should not redefine the outward-facing raster timing. External consumers still expect:

- `active_x`/`active_y` to describe the current **screen** pixel
- `line_start`/`frame_start` to align with screen timing
- `video_de`/`video_hs`/`video_vs` to remain unchanged

Those outputs are part of the module’s contract and are used by the existing focused tests (`rtl/common/primitives/bitmap_text_renderer.sv:39-47`, `testbench/tests/bitmap_text_renderer_test.rs:37-70`, `testbench/tests/bitmap_text_renderer_test.rs:214-249`).

Instead, scroll should be applied only to new internal source-space calculations. Today the module derives:

- `active_x_in_tile = sync_active_x[2:0]`
- `active_tile_column = sync_active_x >> 3`
- `active_tile_row = sync_active_y >> 3`
- glyph row from `sync_active_y[...]`

(`rtl/common/primitives/bitmap_text_renderer.sv:200-203`, `rtl/common/primitives/bitmap_text_renderer.sv:227-241`)

For scrolling, those calculations should instead use:

- `src_x_px = wrap(screen_x + scroll_x_px, map_width_px)`
- `src_y_px = wrap(screen_y + scroll_y_px, map_height_px)`
- `src_x_in_tile = src_x_px[2:0]`
- `src_tile_column = src_x_px / 8`
- `src_tile_row = src_y_px / 8`

and the next-line prefetch should use `wrap(screen_y + 1 + scroll_y_px, map_height_px)` instead of the current direct `latched_active_y + 1` model (`rtl/common/primitives/bitmap_text_renderer.sv:205-210`).

## Register Interface Options

### Option A: Direct top-level ports into the renderer

Add input ports such as:

```systemverilog
input wire logic [SCROLL_X_WIDTH-1:0] scroll_x_px,
input wire logic [SCROLL_Y_WIDTH-1:0] scroll_y_px,
```

Pros:

- smallest RTL surface-area change inside the renderer
- matches the current module style, which is parameterized and stateless except for video pipeline registers

Cons:

- the current production instantiation in `cyclonev_analogue_pocket_top.sv` has no source for those values yet (`rtl/fpga/cyclonev_analogue_pocket/cyclonev_analogue_pocket_top.sv:82-106`)
- another module or peripheral must still own the actual programmable registers

### Option B: Separate MMIO scroll-register peripheral feeding the renderer

Create a new small RTL peripheral that exposes scroll registers on the system bus and routes them into the renderer. This is a more complete system solution, but it is broader than the renderer-only change.

Pros:

- aligned with the rest of the project’s register-driven peripherals
- gives software a natural place to control the background

Cons:

- broader scope than the current issue
- requires top-level system-bus integration, address allocation, and software-facing documentation

### Recommended report conclusion

For implementation planning, the best split is:

1. first add **renderer scroll input ports** and validate behavior in isolation;
2. then add a separate control/register source once the pixel pipeline is correct.

That keeps the first change bounded to the video datapath.

## Scroll Semantics Recommendation

### Recommended axis behavior

- `scroll_x_px`: horizontal source-pixel offset
- `scroll_y_px`: vertical source-pixel offset
- both registers wrap modulo the background dimensions

The internal address equations should be:

```text
src_x_px = (screen_x + scroll_x_px) mod map_width_px
src_y_px = (screen_y + scroll_y_px) mod map_height_px
```

### Recommended update timing

The renderer currently pipelines requests across several cycles using:

- `char_req_valid_d0/d1/d2`
- `char_req_current_tile_d0/d1/d2`
- `font_req_valid_d0/d1/d2`
- `font_req_current_tile_d0/d1/d2`

(`rtl/common/primitives/bitmap_text_renderer.sv:95-109`, `rtl/common/primitives/bitmap_text_renderer.sv:329-348`)

If scroll values are allowed to change every cycle, outstanding ROM requests and buffered tile rows can refer to older scroll coordinates than the current output pixel. That can create tearing or transient mismatches.

The safest first implementation is to **latch the incoming scroll registers on `frame_start`**. Line-latching is also possible, but frame-latching is simpler because it avoids visible within-frame discontinuities and simplifies verification.

## The Core Challenge: Horizontal Pixel Scroll

Vertical scroll is conceptually simple: it changes which tile row and glyph row are addressed for a given screen-space `y`.

Horizontal scroll is harder because the current renderer assumes:

- the first visible screen pixel of a line corresponds to tile pixel 0
- tile boundaries happen whenever `sync_active_x[2:0] == 0`
- the next line can be primed by fetching **tile column 0** during blanking

Those assumptions appear in:

- startup current-tile fetch of column 0 (`rtl/common/primitives/bitmap_text_renderer.sv:224-229`)
- blanking-time fetch of next-line column 0 (`rtl/common/primitives/bitmap_text_renderer.sv:242-251`)
- line-start invalidation of current tile (`rtl/common/primitives/bitmap_text_renderer.sv:321-323`)
- conditional handoff into `current_tile_row_bits` at tile-pixel 0 (`rtl/common/primitives/bitmap_text_renderer.sv:355-372`)

With SNES-style scroll, screen x=0 may correspond to source tile pixel 3, 5, or 7. In that case:

- the first visible pixel of the line must come from a **prefetched current tile row**
- the tile transition does **not** happen at screen x=0
- the next-line blanking fetch must fetch the first **scrolled** source tile, not literal tile column 0

This is the main reason the feature needs a research report before implementation.

## Recommended RTL Refactor

### 1. Separate screen-space and source-space signals

Keep:

- `sync_active_x`, `sync_active_y`
- output `active_x`, `active_y`

Add new internal signals such as:

- `src_x_px`, `src_y_px`
- `src_x_in_tile`
- `src_tile_column`
- `src_tile_row`
- `next_line_src_y_px`

### 2. Introduce explicit wrap helpers

The implementation must **not** rely on width truncation for wrapping.

This matters because the real production dimensions are not powers of two:

- Analogue Pocket instantiates `ACTIVE_WIDTH = 320` and `ACTIVE_HEIGHT = 240` (`rtl/fpga/cyclonev_analogue_pocket/cyclonev_analogue_pocket_top.sv:36-43`, `rtl/fpga/cyclonev_analogue_pocket/cyclonev_analogue_pocket_top.sv:82-95`)
- inside the renderer, the widths are derived from `$clog2`, which gives 9 bits for 320 and 8 bits for 240 (`rtl/common/primitives/bitmap_text_renderer.sv:61-68`)

Naively truncating a sum wraps 320-wide screen coordinates modulo 512, not modulo 320. The same problem applies to 40 tile columns, which would incorrectly wrap modulo 64 if handled only by bit width.

### 3. Decouple map size from visible size

If the goal is true background scrolling rather than “wrap around the visible screen contents,” the renderer should gain separate parameters:

- `MAP_WIDTH_TILES`
- `MAP_HEIGHT_TILES`

and derive:

- `MAP_WIDTH_PX = MAP_WIDTH_TILES * TILE_WIDTH`
- `MAP_HEIGHT_PX = MAP_HEIGHT_TILES * TILE_HEIGHT`
- `CHARMAP_DEPTH = MAP_WIDTH_TILES * MAP_HEIGHT_TILES`

Without this change, scrolling can still work, but it only wraps around the currently visible screen-sized map.

### 4. Rework line-start priming

The first active pixel of each line must already have the correct source tile row available in a “current” buffer, even when `scroll_x[2:0] != 0`.

A practical approach is:

- during blanking, fetch the first scrolled tile row for the next line into a dedicated line-start/current buffer
- also fetch or retain the second tile row as the “next” tile buffer
- on line start, do **not** blindly clear `current_tile_valid`; instead, promote the prepared line-start buffer into the current-tile position

This preserves the existing philosophy of hiding ROM latency with prefetch, but removes the assumption that the line begins on a tile boundary.

### 5. Keep the registered output alignment unchanged

The final output stage that delays:

- `video_de`
- `video_hs`
- `video_vs`
- `line_start`
- `frame_start`
- `active_x`
- `active_y`
- `pixel_on`

should remain structurally intact (`rtl/common/primitives/bitmap_text_renderer.sv:303-312`).

This stage is not the problem; the issue is upstream address/tile-row preparation.

## Integration Impact

### Current production use

The only production-style instantiation found in the repository is the Analogue Pocket top-level wrapper:

- `rtl/fpga/cyclonev_analogue_pocket/cyclonev_analogue_pocket_top.sv:82-106`

It drives the renderer with:

- `clk_video`
- `video_rst`
- ROM init parameters

and consumes only:

- `video_de`
- `video_hs`
- `video_vs`
- `pixel_on`

There is currently no register/control path for background scroll. Any complete end-to-end feature will therefore require either:

- hardwired test values for initial FPGA experiments, or
- a follow-up control/register integration change.

### Focused verification path already exists

The repository already has a standalone wrapper and a focused Rust integration test for the renderer:

- wrapper: `rtl/common/wrappers/bitmap_text_renderer_test_wrapper.sv:2-37`
- runtime binding: `riscv_core/src/lib.rs:250-254`, `riscv_core/src/lib.rs:595-603`
- tests: `testbench/tests/bitmap_text_renderer_test.rs:1-249`

That means the best initial implementation path is to extend the isolated renderer test environment first, before touching any board-level integration.

## Verification Strategy

### Existing test coverage

The current tests already validate:

- coordinate/pixel alignment (`testbench/tests/bitmap_text_renderer_test.rs:81-113`)
- first-frame startup priming (`testbench/tests/bitmap_text_renderer_test.rs:115-141`)
- steady-state full-frame bitmap correctness (`testbench/tests/bitmap_text_renderer_test.rs:143-183`)
- blanking behavior (`testbench/tests/bitmap_text_renderer_test.rs:185-212`)
- sync/control output alignment (`testbench/tests/bitmap_text_renderer_test.rs:214-249`)

This is an excellent base for scroll testing because the module already has deterministic expected-pixel checks.

### Required test extensions

1. **Add scroll inputs to the wrapper** so tests can set nonzero offsets (`rtl/common/wrappers/bitmap_text_renderer_test_wrapper.sv:2-37`).
2. **Extend the Rust expected-pixel helper** to compute source-space lookups using wrapped `x + scroll_x` and `y + scroll_y` instead of the current direct tile lookup (`testbench/tests/bitmap_text_renderer_test.rs:73-79`).
3. Add focused cases for:
   - horizontal scroll by 1..7 pixels
   - horizontal scroll by exactly 8 pixels
   - vertical scroll by 1..7 pixels
   - combined X/Y scroll
   - right-edge wrap
   - bottom-edge wrap
   - reset/startup behavior with nonzero scroll
   - scroll update timing (frame-latched behavior is easiest to assert)

### Important test gap to avoid

The current wrapper uses a **16x16** active area (`rtl/common/wrappers/bitmap_text_renderer_test_wrapper.sv:15-18`), which is a power-of-two dimension. That is convenient for simple tests, but it can accidentally hide incorrect wrap logic.

If implementation testing is added later, at least one focused wrapper should use a non-power-of-two active/map width such as 24 or 40 pixels wide so broken bit-truncation wrap logic cannot pass by accident.

## Key Risks and Edge Cases

### 1. Incorrect wrap semantics

This is the highest-risk functional bug. Width truncation is insufficient because the production dimensions are not powers of two (`rtl/common/primitives/bitmap_text_renderer.sv:61-68`, `rtl/fpga/cyclonev_analogue_pocket/cyclonev_analogue_pocket_top.sv:36-43`).

### 2. First-pixel-of-line corruption

If line-start priming is not redesigned, `scroll_x[2:0] != 0` will likely cause wrong pixels or blank pixels at the beginning of each line because `current_tile_valid` is deliberately cleared on line start today (`rtl/common/primitives/bitmap_text_renderer.sv:321-323`).

### 3. Mid-frame tearing when registers change live

Because the renderer carries requests through several valid stages and row buffers (`rtl/common/primitives/bitmap_text_renderer.sv:329-372`), asynchronous register updates can make buffered data disagree with the current coordinate transform. Frame-latching avoids this.

### 4. Screen-sized map limitation

If the map size remains tied to `ACTIVE_WIDTH`/`ACTIVE_HEIGHT`, scrolling will still function, but it will only wrap across a background exactly as large as the visible area (`rtl/common/primitives/bitmap_text_renderer.sv:63-68`). That may be acceptable for a first step, but it should be called out explicitly in any implementation plan.

## Recommended Next Step

The next artifact after this research document should be a focused implementation plan that does the following in order:

1. add renderer scroll input ports and internal source-coordinate logic;
2. add explicit map wrap helpers and separate map-size parameters;
3. rework line-start tile-row priming to support `scroll_x[2:0] != 0`;
4. extend the wrapper/tests to cover scrolled rendering;
5. only then consider adding a real bus-visible scroll-register peripheral.

## Bottom Line

Adding a **vertical** scroll register is a modest extension of the current coordinate path. Adding a truly **SNES-style horizontal pixel scroll register** requires a more deliberate change to the renderer’s line-start/prefetch model. The existing module is already structured well enough to support that work, but it should be treated as a pipeline/prefetch redesign inside a narrow area of the module rather than as a trivial address offset patch.
