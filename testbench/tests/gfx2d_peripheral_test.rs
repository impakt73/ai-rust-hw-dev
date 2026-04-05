use riscv_core::AsDynamicVerilatedModel;
use riscv_core::{create_gfx2d_peripheral_runtime, Gfx2dPeripheralTestWrapper};
use riscv_shared::bus::{gfx2d_control_addr, GFX2D_CONTROL_ENABLE};

const GFX2D_BASE_ADDR: u32 = 0x3000_0000;
const GFX2D_SCROLL_X_ADDR: u32 = GFX2D_BASE_ADDR;
const GFX2D_SCROLL_Y_ADDR: u32 = GFX2D_BASE_ADDR + 4;
const GFX2D_CONTROL_ADDR: u32 = gfx2d_control_addr();
const GFX2D_FRAME_INDEX_ADDR: u32 = GFX2D_BASE_ADDR + 12;
const GFX2D_CHAR_MAP_BASE_ADDR: u32 = GFX2D_BASE_ADDR + 0x1000;
const GFX2D_FONT_BASE_ADDR: u32 = GFX2D_BASE_ADDR + 0x2000;
const GFX2D_PALETTE_BASE_ADDR: u32 = GFX2D_BASE_ADDR + 0x6000;
const MEM_SIZE_BYTE: u8 = 0;
const MEM_SIZE_HALFWORD: u8 = 1;
const MEM_SIZE_WORD: u8 = 2;
const GFX2D_ACCESS_TIMEOUT_CYCLES: usize = 64;

const GFX2D_H_TOTAL: usize = 26;
const GFX2D_V_TOTAL: usize = 19;
const GFX2D_FRAME_CYCLES: usize = GFX2D_H_TOTAL * GFX2D_V_TOTAL;
const GFX2D_ACTIVE_WIDTH: u8 = 16;
const GFX2D_ACTIVE_HEIGHT: u8 = 16;
const GFX2D_SCROLL_X_WRAP_MASK: u8 = GFX2D_ACTIVE_WIDTH - 1;
const GFX2D_SCROLL_Y_WRAP_MASK: u8 = GFX2D_ACTIVE_HEIGHT - 1;
const GFX2D_CHAR_MAP: [u8; 4] = [1, 2, 3, 4];
const GFX2D_FONT_ROWS: [[u8; 8]; 5] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x81, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81],
    [0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00],
    [0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0],
    [0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F],
];

macro_rules! clock_cycle {
    ($dut:expr) => {
        // Keep both domains phase-aligned in this unit test so bus writes and
        // raster progress are deterministic while still exercising the CDC path.
        $dut.sys_clk = 0;
        $dut.video_clk = 0;
        $dut.eval();
        $dut.sys_clk = 1;
        $dut.video_clk = 1;
        $dut.eval();
        $dut.sys_clk = 0;
        $dut.video_clk = 0;
        $dut.eval();
    };
}

fn reset(dut: &mut Gfx2dPeripheralTestWrapper) {
    dut.rst = 1;
    dut.mem_a_addr = 0;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = 0;
    dut.mem_a_valid = 0;
    dut.mem_d_ready = 0;

    for _ in 0..6 {
        clock_cycle!(dut);
    }

    dut.rst = 0;
    for _ in 0..6 {
        clock_cycle!(dut);
    }
}

fn wait_for_response(dut: &mut Gfx2dPeripheralTestWrapper, max_cycles: usize) -> usize {
    for elapsed_cycles in 0..max_cycles {
        if dut.mem_d_valid != 0 {
            return elapsed_cycles;
        }
        clock_cycle!(dut);
    }

    panic!("timed out waiting for gfx2d peripheral response");
}

fn write_access_with_size(
    dut: &mut Gfx2dPeripheralTestWrapper,
    addr: u32,
    wdata: u32,
    size: u8,
) -> usize {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = wdata;
    dut.mem_a_we = 1;
    dut.mem_a_size = size;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "expected system bus request to be accepted"
    );

    let mut elapsed_cycles = 0;
    clock_cycle!(dut);
    elapsed_cycles += 1;
    dut.mem_a_valid = 0;
    dut.mem_a_we = 0;
    dut.eval();

    elapsed_cycles += wait_for_response(dut, GFX2D_ACCESS_TIMEOUT_CYCLES);
    assert_eq!(
        dut.mem_d_rdata, 0,
        "writes should acknowledge with zero data"
    );

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    elapsed_cycles += 1;
    dut.mem_d_ready = 0;
    dut.eval();

    elapsed_cycles
}

fn write_access(dut: &mut Gfx2dPeripheralTestWrapper, addr: u32, wdata: u32) -> usize {
    write_access_with_size(dut, addr, wdata, MEM_SIZE_WORD)
}

fn set_video_enable(dut: &mut Gfx2dPeripheralTestWrapper, enabled: bool) -> usize {
    write_access(
        dut,
        GFX2D_CONTROL_ADDR,
        if enabled { GFX2D_CONTROL_ENABLE } else { 0 },
    )
}

fn read_access_with_size(dut: &mut Gfx2dPeripheralTestWrapper, addr: u32, size: u8) -> u32 {
    dut.mem_a_addr = addr;
    dut.mem_a_wdata = 0;
    dut.mem_a_we = 0;
    dut.mem_a_size = size;
    dut.mem_a_valid = 1;
    dut.eval();

    assert_eq!(
        dut.mem_a_ready, 1,
        "expected system bus request to be accepted"
    );

    clock_cycle!(dut);
    dut.mem_a_valid = 0;
    dut.eval();

    wait_for_response(dut, GFX2D_ACCESS_TIMEOUT_CYCLES);
    let rdata = dut.mem_d_rdata;

    dut.mem_d_ready = 1;
    clock_cycle!(dut);
    dut.mem_d_ready = 0;
    dut.eval();

    rdata
}

fn read_access(dut: &mut Gfx2dPeripheralTestWrapper, addr: u32) -> u32 {
    read_access_with_size(dut, addr, MEM_SIZE_WORD)
}

fn font_addr(glyph: u8, row: u8, col: u8) -> u32 {
    GFX2D_FONT_BASE_ADDR + (u32::from(glyph) << 6) + (u32::from(row) << 3) + u32::from(col)
}

fn palette_addr(index: u8) -> u32 {
    GFX2D_PALETTE_BASE_ADDR + (u32::from(index) << 2)
}

fn load_test_pattern(dut: &mut Gfx2dPeripheralTestWrapper) {
    for (index, glyph) in GFX2D_CHAR_MAP.iter().enumerate() {
        write_access_with_size(
            dut,
            GFX2D_CHAR_MAP_BASE_ADDR + u32::try_from(index).unwrap(),
            u32::from(*glyph),
            MEM_SIZE_BYTE,
        );
    }

    for (glyph, rows) in GFX2D_FONT_ROWS.iter().enumerate() {
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..8 {
                let palette_index = u32::from(((bits >> (7 - col)) & 1) != 0);
                write_access_with_size(
                    dut,
                    font_addr(glyph as u8, row as u8, col as u8),
                    palette_index,
                    MEM_SIZE_BYTE,
                );
            }
        }
    }

    write_access(dut, palette_addr(0), 0xAA00_0000);
    write_access(dut, palette_addr(1), 0x12FF_FFFF);
}

fn wait_for_active_frame_start(dut: &mut Gfx2dPeripheralTestWrapper, occurrence: usize) {
    let mut seen = 0;
    let mut saw_vsync_pulse = false;

    for _ in 0..(GFX2D_FRAME_CYCLES * 4) {
        clock_cycle!(dut);
        if dut.video_vs == 0 {
            saw_vsync_pulse = true;
        }
        if saw_vsync_pulse && dut.video_vs == 1 && dut.video_de == 1 {
            seen += 1;
            if seen == occurrence {
                return;
            }
            saw_vsync_pulse = false;
        }
    }

    panic!("timed out waiting for active frame start occurrence {occurrence}");
}

fn wait_for_next_active_frame_start(dut: &mut Gfx2dPeripheralTestWrapper) {
    wait_for_active_frame_start(dut, 1);
}

fn capture_active_frame_pixels(
    dut: &mut Gfx2dPeripheralTestWrapper,
    occurrence: usize,
) -> Vec<u32> {
    let width = usize::from(GFX2D_ACTIVE_WIDTH);
    let height = usize::from(GFX2D_ACTIVE_HEIGHT);
    let mut pixels = Vec::with_capacity(width * height);

    wait_for_active_frame_start(dut, occurrence);

    for row in 0..height {
        for col in 0..width {
            assert_eq!(
                dut.video_de, 1,
                "expected active video at raster coordinate ({col}, {row})"
            );
            pixels.push(dut.video_rgb);

            if col + 1 != width {
                clock_cycle!(dut);
            }
        }

        if row + 1 != height {
            clock_cycle!(dut);
            for _ in 0..GFX2D_H_TOTAL {
                if dut.video_de == 1 {
                    break;
                }
                clock_cycle!(dut);
            }

            assert_eq!(
                dut.video_de,
                1,
                "timed out waiting for active video on row {}",
                row + 1
            );
        }
    }

    pixels
}

fn capture_frame_de_mask(dut: &mut Gfx2dPeripheralTestWrapper, occurrence: usize) -> Vec<bool> {
    let mut de_mask = Vec::with_capacity(GFX2D_FRAME_CYCLES);

    wait_for_active_frame_start(dut, occurrence);

    for _ in 0..GFX2D_FRAME_CYCLES {
        de_mask.push(dut.video_de == 1);
        clock_cycle!(dut);
    }

    de_mask
}

fn active_frame_pixel(pixels: &[u32], x: u8, y: u8) -> u32 {
    pixels[(usize::from(y) * usize::from(GFX2D_ACTIVE_WIDTH)) + usize::from(x)]
}

fn expected_pixel(x: u8, y: u8) -> u32 {
    let tile_x = usize::from(x / 8);
    let tile_y = usize::from(y / 8);
    let glyph = usize::from(GFX2D_CHAR_MAP[(tile_y * 2) + tile_x]);
    let glyph_row = GFX2D_FONT_ROWS[glyph][usize::from(y % 8)];
    if ((glyph_row >> (7 - (x % 8))) & 1) != 0 {
        0xFF_FF_FF
    } else {
        0x00_00_00
    }
}

fn expected_scrolled_pixel(x: u8, y: u8, scroll_x: u8, scroll_y: u8) -> u32 {
    expected_pixel(
        (x.wrapping_add(scroll_x)) & GFX2D_SCROLL_X_WRAP_MASK,
        (y.wrapping_add(scroll_y)) & GFX2D_SCROLL_Y_WRAP_MASK,
    )
}

fn verify_active_pixels_from_current_frame(
    dut: &mut Gfx2dPeripheralTestWrapper,
    start_cycle_in_frame: usize,
    active_pixels_to_check: usize,
    scroll_x: u8,
    scroll_y: u8,
) {
    let mut cycle_in_frame = start_cycle_in_frame;
    let mut checked = 0usize;

    while checked < active_pixels_to_check {
        let row = cycle_in_frame / GFX2D_H_TOTAL;
        let col = cycle_in_frame % GFX2D_H_TOTAL;

        if row < usize::from(GFX2D_ACTIVE_HEIGHT) && col < usize::from(GFX2D_ACTIVE_WIDTH) {
            assert_eq!(
                dut.video_de, 1,
                "expected active video while validating current frame"
            );
            assert_eq!(
                dut.video_rgb,
                expected_scrolled_pixel(col as u8, row as u8, scroll_x, scroll_y),
                "unexpected pixel during same-frame validation at output coordinate ({col}, {row})"
            );
            checked += 1;
        }

        cycle_in_frame += 1;
        clock_cycle!(dut);
    }
}

fn verify_black_active_pixels_from_current_frame(
    dut: &mut Gfx2dPeripheralTestWrapper,
    start_cycle_in_frame: usize,
    active_pixels_to_check: usize,
) {
    let mut cycle_in_frame = start_cycle_in_frame;
    let mut checked = 0usize;

    while checked < active_pixels_to_check {
        let row = cycle_in_frame / GFX2D_H_TOTAL;
        let col = cycle_in_frame % GFX2D_H_TOTAL;

        if row < usize::from(GFX2D_ACTIVE_HEIGHT) && col < usize::from(GFX2D_ACTIVE_WIDTH) {
            assert_eq!(
                dut.video_de, 1,
                "expected active video while validating current frame"
            );
            assert_eq!(
                dut.video_rgb, 0,
                "expected black pixel during same-frame validation at output coordinate ({col}, {row})"
            );
            checked += 1;
        }

        cycle_in_frame += 1;
        clock_cycle!(dut);
    }
}

#[test]
fn test_gfx2d_scroll_registers_reset_low_and_read_back() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);

    assert_eq!(read_access(&mut dut, GFX2D_SCROLL_X_ADDR), 0);
    assert_eq!(read_access(&mut dut, GFX2D_SCROLL_Y_ADDR), 0);
}

#[test]
fn test_gfx2d_control_register_reset_low_and_read_back() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);

    assert_eq!(read_access(&mut dut, GFX2D_CONTROL_ADDR), 0);

    write_access(&mut dut, GFX2D_CONTROL_ADDR, u32::MAX);
    assert_eq!(
        read_access(&mut dut, GFX2D_CONTROL_ADDR),
        GFX2D_CONTROL_ENABLE,
        "reserved control bits must read back as zero"
    );

    set_video_enable(&mut dut, false);
    assert_eq!(read_access(&mut dut, GFX2D_CONTROL_ADDR), 0);
}

#[test]
fn test_gfx2d_bus_writes_change_rendered_scroll_output() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);
    load_test_pattern(&mut dut);
    set_video_enable(&mut dut, true);

    let baseline_pixels = capture_active_frame_pixels(&mut dut, 2);
    for y in 0..GFX2D_ACTIVE_HEIGHT {
        for x in 0..GFX2D_ACTIVE_WIDTH {
            assert_eq!(
                active_frame_pixel(&baseline_pixels, x, y),
                expected_pixel(x, y),
                "unexpected baseline pixel at ({x}, {y})"
            );
        }
    }

    wait_for_active_frame_start(&mut dut, 1);
    let elapsed_cycles = write_access(&mut dut, GFX2D_SCROLL_X_ADDR, 1)
        + write_access(&mut dut, GFX2D_SCROLL_Y_ADDR, 1);

    verify_active_pixels_from_current_frame(&mut dut, elapsed_cycles % GFX2D_FRAME_CYCLES, 8, 0, 0);
    assert_eq!(read_access(&mut dut, GFX2D_SCROLL_X_ADDR), 1);
    assert_eq!(read_access(&mut dut, GFX2D_SCROLL_Y_ADDR), 1);

    let pixels = capture_active_frame_pixels(&mut dut, 1);
    for y in 0..GFX2D_ACTIVE_HEIGHT {
        for x in 0..GFX2D_ACTIVE_WIDTH {
            assert_eq!(
                active_frame_pixel(&pixels, x, y),
                expected_scrolled_pixel(x, y, 1, 1),
                "unexpected pixel at ({x}, {y}) after frame-latched scroll update"
            );
        }
    }

    for (scroll_x, scroll_y) in [(1u8, 0u8), (15u8, 0u8), (0u8, 1u8), (9u8, 10u8)] {
        write_access(&mut dut, GFX2D_SCROLL_X_ADDR, u32::from(scroll_x));
        write_access(&mut dut, GFX2D_SCROLL_Y_ADDR, u32::from(scroll_y));
        assert_eq!(
            read_access(&mut dut, GFX2D_SCROLL_X_ADDR),
            u32::from(scroll_x)
        );
        assert_eq!(
            read_access(&mut dut, GFX2D_SCROLL_Y_ADDR),
            u32::from(scroll_y)
        );

        let pixels = capture_active_frame_pixels(&mut dut, 2);
        for y in 0..GFX2D_ACTIVE_HEIGHT {
            for x in 0..GFX2D_ACTIVE_WIDTH {
                assert_eq!(
                    active_frame_pixel(&pixels, x, y),
                    expected_scrolled_pixel(x, y, scroll_x, scroll_y),
                    "unexpected pixel at ({x}, {y}) with scroll_x={scroll_x} scroll_y={scroll_y}"
                );
            }
        }

        let mut observed_blanking = false;
        let mut saw_hsync_pulse = false;
        let mut saw_vsync_pulse = false;
        for _ in 0..GFX2D_FRAME_CYCLES {
            if dut.video_de == 0 {
                observed_blanking = true;
                assert_eq!(dut.video_rgb, 0, "video_rgb must stay low during blanking");
            }
            if dut.video_hs == 0 {
                saw_hsync_pulse = true;
            }
            if dut.video_vs == 0 {
                saw_vsync_pulse = true;
            }
            clock_cycle!(dut);
        }

        assert!(observed_blanking, "expected blanking interval");
        assert!(saw_hsync_pulse, "expected horizontal sync pulse");
        assert!(saw_vsync_pulse, "expected vertical sync pulse");
        assert_eq!(dut.video_skip, 0, "video_skip should stay low");
    }
}

#[test]
fn test_gfx2d_video_disable_forces_black_active_pixels_without_changing_de_timing() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);
    load_test_pattern(&mut dut);

    let reset_disabled_pixels = capture_active_frame_pixels(&mut dut, 2);
    assert!(
        reset_disabled_pixels.iter().all(|&pixel| pixel == 0),
        "video must start disabled after reset"
    );

    set_video_enable(&mut dut, true);

    let enabled_pixels = capture_active_frame_pixels(&mut dut, 2);
    let enabled_de_mask = capture_frame_de_mask(&mut dut, 1);
    assert!(
        enabled_pixels.iter().any(|&pixel| pixel != 0),
        "expected non-black pixels while video is enabled"
    );

    wait_for_active_frame_start(&mut dut, 1);
    let elapsed_cycles = set_video_enable(&mut dut, false);
    assert_eq!(read_access(&mut dut, GFX2D_CONTROL_ADDR), 0);
    verify_active_pixels_from_current_frame(&mut dut, elapsed_cycles % GFX2D_FRAME_CYCLES, 8, 0, 0);

    let disabled_pixels = capture_active_frame_pixels(&mut dut, 2);
    let disabled_de_mask = capture_frame_de_mask(&mut dut, 1);
    assert!(
        disabled_pixels.iter().all(|&pixel| pixel == 0),
        "disabling video must force active pixels to black"
    );

    assert_eq!(
        disabled_de_mask, enabled_de_mask,
        "video disable must preserve DE timing"
    );

    assert_eq!(
        disabled_de_mask.iter().filter(|&&de| de).count(),
        usize::from(GFX2D_ACTIVE_WIDTH) * usize::from(GFX2D_ACTIVE_HEIGHT),
        "video disable must preserve DE active-window timing"
    );

    wait_for_active_frame_start(&mut dut, 1);
    let elapsed_cycles = set_video_enable(&mut dut, true);
    assert_eq!(read_access(&mut dut, GFX2D_CONTROL_ADDR), GFX2D_CONTROL_ENABLE);
    verify_black_active_pixels_from_current_frame(&mut dut, elapsed_cycles % GFX2D_FRAME_CYCLES, 8);

    let reenabled_pixels = capture_active_frame_pixels(&mut dut, 2);
    assert!(
        reenabled_pixels.iter().any(|&pixel| pixel != 0),
        "re-enabling video must restore active pixels on the next frame"
    );
}

#[test]
fn test_gfx2d_subword_mmio_accesses_are_ignored() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);

    write_access(&mut dut, GFX2D_SCROLL_X_ADDR, 0x1122_3344);
    write_access_with_size(&mut dut, GFX2D_SCROLL_X_ADDR, 0x0000_00AA, MEM_SIZE_BYTE);
    write_access_with_size(
        &mut dut,
        GFX2D_SCROLL_X_ADDR,
        0x0000_BBCC,
        MEM_SIZE_HALFWORD,
    );
    write_access_with_size(
        &mut dut,
        GFX2D_SCROLL_X_ADDR + 1,
        0x5566_7788,
        MEM_SIZE_WORD,
    );

    assert_eq!(
        read_access(&mut dut, GFX2D_SCROLL_X_ADDR),
        0x1122_3344,
        "subword and unaligned accesses must not modify MMIO registers"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_SCROLL_X_ADDR, MEM_SIZE_BYTE),
        0,
        "subword MMIO reads should return zero"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_SCROLL_X_ADDR, MEM_SIZE_HALFWORD),
        0,
        "subword MMIO reads should return zero"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_SCROLL_X_ADDR + 1, MEM_SIZE_WORD),
        0,
        "unaligned MMIO reads should return zero"
    );

    write_access(&mut dut, GFX2D_CONTROL_ADDR, GFX2D_CONTROL_ENABLE);
    write_access_with_size(&mut dut, GFX2D_CONTROL_ADDR, 0x0000_0000, MEM_SIZE_BYTE);
    write_access_with_size(&mut dut, GFX2D_CONTROL_ADDR, 0x0000_0000, MEM_SIZE_HALFWORD);
    write_access_with_size(&mut dut, GFX2D_CONTROL_ADDR + 1, 0x0000_0000, MEM_SIZE_WORD);
    assert_eq!(
        read_access(&mut dut, GFX2D_CONTROL_ADDR),
        GFX2D_CONTROL_ENABLE,
        "subword and unaligned accesses must not modify the control register"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_CONTROL_ADDR, MEM_SIZE_BYTE),
        0,
        "subword control reads should return zero"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_CONTROL_ADDR, MEM_SIZE_HALFWORD),
        0,
        "subword control reads should return zero"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_CONTROL_ADDR + 1, MEM_SIZE_WORD),
        0,
        "unaligned control reads should return zero"
    );

    let frame_index_before_write = read_access(&mut dut, GFX2D_FRAME_INDEX_ADDR);
    write_access(&mut dut, GFX2D_FRAME_INDEX_ADDR, 0xDEAD_BEEF);
    assert_eq!(
        read_access(&mut dut, GFX2D_FRAME_INDEX_ADDR),
        frame_index_before_write,
        "frame index writes must be ignored"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_FRAME_INDEX_ADDR, MEM_SIZE_HALFWORD),
        0,
        "subword frame index reads should return zero"
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_FRAME_INDEX_ADDR + 1, MEM_SIZE_WORD),
        0,
        "unaligned frame index reads should return zero"
    );
}

#[test]
fn test_gfx2d_ram_windows_are_write_only() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);
    load_test_pattern(&mut dut);
    set_video_enable(&mut dut, true);

    write_access_with_size(
        &mut dut,
        GFX2D_CHAR_MAP_BASE_ADDR,
        0x0000_0034,
        MEM_SIZE_BYTE,
    );
    assert_eq!(
        read_access_with_size(&mut dut, GFX2D_CHAR_MAP_BASE_ADDR, MEM_SIZE_BYTE),
        0,
        "char map RAM must be write-only from the CPU"
    );
    write_access(&mut dut, GFX2D_CHAR_MAP_BASE_ADDR, 0xAABB_CCDD);
    assert_eq!(
        read_access(&mut dut, GFX2D_CHAR_MAP_BASE_ADDR),
        0,
        "word reads from char map RAM must be dropped"
    );

    write_access_with_size(&mut dut, font_addr(2, 3, 4), 0x0000_0056, MEM_SIZE_BYTE);
    assert_eq!(
        read_access_with_size(&mut dut, font_addr(2, 3, 4), MEM_SIZE_BYTE),
        0,
        "font RAM must be write-only from the CPU"
    );
    write_access_with_size(&mut dut, font_addr(2, 3, 4), 0x0000_BBCC, MEM_SIZE_HALFWORD);
    assert_eq!(
        read_access_with_size(&mut dut, font_addr(2, 3, 4), MEM_SIZE_HALFWORD),
        0,
        "non-byte reads from font RAM must be dropped"
    );

    write_access(&mut dut, palette_addr(7), 0xAB12_3456);
    assert_eq!(
        read_access(&mut dut, palette_addr(7)),
        0,
        "palette RAM must be write-only from the CPU"
    );

    let baseline_pixels = capture_active_frame_pixels(&mut dut, 2);

    write_access_with_size(&mut dut, palette_addr(7), 0x0000_00EE, MEM_SIZE_BYTE);
    write_access_with_size(&mut dut, palette_addr(7), 0x0000_DDEE, MEM_SIZE_HALFWORD);
    write_access_with_size(&mut dut, palette_addr(7) + 1, 0x5566_7788, MEM_SIZE_WORD);
    assert_eq!(
        read_access_with_size(&mut dut, palette_addr(7), MEM_SIZE_BYTE),
        0,
        "subword palette reads must be dropped"
    );
    assert_eq!(
        read_access_with_size(&mut dut, palette_addr(7) + 1, MEM_SIZE_WORD),
        0,
        "unaligned palette reads must be dropped"
    );

    let pixels = capture_active_frame_pixels(&mut dut, 2);
    assert_eq!(
        pixels, baseline_pixels,
        "unsupported writes must not perturb the renderer-visible RAM contents"
    );
}

#[test]
fn test_gfx2d_frame_index_advances_once_per_frame() {
    let runtime = create_gfx2d_peripheral_runtime().expect("Failed to create gfx2d runtime");
    let mut dut = runtime
        .create_model_simple::<Gfx2dPeripheralTestWrapper>()
        .expect("Failed to create gfx2d model");

    reset(&mut dut);

    let mut previous_frame_index = read_access(&mut dut, GFX2D_FRAME_INDEX_ADDR);
    write_access(&mut dut, GFX2D_FRAME_INDEX_ADDR, 0x1234_5678);
    assert_eq!(
        read_access(&mut dut, GFX2D_FRAME_INDEX_ADDR),
        previous_frame_index,
        "frame index must stay read-only"
    );

    for expected_increment in 1..=3 {
        wait_for_next_active_frame_start(&mut dut);
        let current_frame_index = read_access(&mut dut, GFX2D_FRAME_INDEX_ADDR);
        assert_eq!(
            current_frame_index,
            previous_frame_index + 1,
            "frame index should advance once per frame (step {expected_increment})"
        );
        previous_frame_index = current_frame_index;
    }
}
