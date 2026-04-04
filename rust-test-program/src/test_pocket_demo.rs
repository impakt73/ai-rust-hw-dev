#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use riscv_rt::entry;
use riscv_shared::bus::{
    audiosys_control_addr, audiosys_tuning_word_addr, gamepad_state_addr, AUDIOSYS_CONTROL_ENABLE,
    GAMEPAD_DPAD_DOWN, GAMEPAD_DPAD_LEFT, GAMEPAD_DPAD_RIGHT, GAMEPAD_DPAD_UP, GAMEPAD_TRIG_L,
    GAMEPAD_TRIG_R, GFX2D_BASE, GFX2D_CHAR_MAP_OFFSET, GFX2D_CHAR_MAP_SIZE, GFX2D_FONT_OFFSET,
    GFX2D_FRAME_INDEX_OFFSET, GFX2D_PALETTE_OFFSET, GFX2D_SCROLL_X_OFFSET, GFX2D_SCROLL_Y_OFFSET,
};

const INITIAL_TUNING_WORD: u32 = 615_165;
const AUDIO_TUNING_STEP: i32 = 1_024;

const TILE_COLUMNS: u32 = 32;
const TILE_PIXELS: u32 = 8 * 8;
const TILE_ZERO: u8 = 0;
const TILE_ONE: u8 = 1;
const PALETTE_ORANGE: u32 = 0x00FF_A500;
const PALETTE_TEAL: u32 = 0x0000_8080;

struct DemoState {
    frame_index: u32,
    scroll_x: u32,
    scroll_y: u32,
    tuning_word: u32,
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    common::default_panic_handler(info)
}

#[inline(never)]
fn read_u32(addr: u32) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

#[inline(never)]
fn write_u32(addr: u32, value: u32) {
    unsafe {
        write_volatile(addr as *mut u32, value);
    }
}

#[inline(never)]
fn write_u8(addr: u32, value: u8) {
    unsafe {
        write_volatile(addr as *mut u8, value);
    }
}

fn initialize_demo() -> DemoState {
    let tuning_word = INITIAL_TUNING_WORD;
    write_u32(audiosys_tuning_word_addr(), tuning_word);
    write_u32(audiosys_control_addr(), AUDIOSYS_CONTROL_ENABLE);

    for index in 0..GFX2D_CHAR_MAP_SIZE {
        let x = index % TILE_COLUMNS;
        let y = index / TILE_COLUMNS;
        let tile_id = if (x + y).is_multiple_of(2) {
            TILE_ZERO
        } else {
            TILE_ONE
        };
        write_u8(GFX2D_BASE + GFX2D_CHAR_MAP_OFFSET + index, tile_id);
    }

    for index in 0..TILE_PIXELS {
        write_u8(GFX2D_BASE + GFX2D_FONT_OFFSET + index, TILE_ZERO);
        write_u8(
            GFX2D_BASE + GFX2D_FONT_OFFSET + TILE_PIXELS + index,
            TILE_ONE,
        );
    }

    write_u32(GFX2D_BASE + GFX2D_PALETTE_OFFSET, PALETTE_ORANGE);
    write_u32(GFX2D_BASE + GFX2D_PALETTE_OFFSET + 4, PALETTE_TEAL);

    DemoState {
        frame_index: read_u32(GFX2D_BASE + GFX2D_FRAME_INDEX_OFFSET),
        scroll_x: read_u32(GFX2D_BASE + GFX2D_SCROLL_X_OFFSET),
        scroll_y: read_u32(GFX2D_BASE + GFX2D_SCROLL_Y_OFFSET),
        tuning_word,
    }
}

fn wait_for_next_frame(previous_frame_index: u32) -> u32 {
    loop {
        let frame_index = read_u32(GFX2D_BASE + GFX2D_FRAME_INDEX_OFFSET);
        if frame_index != previous_frame_index {
            return frame_index;
        }
    }
}

fn button_delta(input_state: u32, negative_mask: u32, positive_mask: u32, step: i32) -> i32 {
    let negative = if input_state & negative_mask != 0 {
        step
    } else {
        0
    };
    let positive = if input_state & positive_mask != 0 {
        step
    } else {
        0
    };

    positive - negative
}

#[entry]
fn main() -> ! {
    let mut state = initialize_demo();

    loop {
        state.frame_index = wait_for_next_frame(state.frame_index);

        let input_state = read_u32(gamepad_state_addr());
        let scroll_x_delta = button_delta(input_state, GAMEPAD_DPAD_LEFT, GAMEPAD_DPAD_RIGHT, 1);
        let scroll_y_delta = button_delta(input_state, GAMEPAD_DPAD_UP, GAMEPAD_DPAD_DOWN, 1);
        let tuning_delta = button_delta(
            input_state,
            GAMEPAD_TRIG_L,
            GAMEPAD_TRIG_R,
            AUDIO_TUNING_STEP,
        );

        state.scroll_x = state.scroll_x.wrapping_add_signed(scroll_x_delta);
        state.scroll_y = state.scroll_y.wrapping_add_signed(scroll_y_delta);
        state.tuning_word = state.tuning_word.wrapping_add_signed(tuning_delta);

        write_u32(GFX2D_BASE + GFX2D_SCROLL_X_OFFSET, state.scroll_x);
        write_u32(GFX2D_BASE + GFX2D_SCROLL_Y_OFFSET, state.scroll_y);
        write_u32(audiosys_tuning_word_addr(), state.tuning_word);
    }
}
