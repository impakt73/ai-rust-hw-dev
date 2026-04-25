#![no_std]
#![no_main]

mod common;

#[global_allocator]
static HEAP: common::Heap = common::Heap::empty();

use core::cell::Cell;
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use critical_section::Mutex;
use riscv::result::{Error, Result};
use riscv::{ExternalInterruptNumber, InterruptNumber};
use riscv_rt::{entry, external_interrupt};
use riscv_shared::bus::gfx2d_control_addr;
use riscv_shared::bus::{
    audiosys_fifo_pack_stereo_sample, audiosys_fifo_sample_addr, audiosys_fifo_space_addr,
    audiosys_mode_addr, gamepad_state_addr, interrupt_ctrl_claim_addr,
    interrupt_ctrl_complete_addr, interrupt_ctrl_enable_addr, AUDIOSYS_MODE_FIFO,
    GAMEPAD_DPAD_DOWN, GAMEPAD_DPAD_LEFT, GAMEPAD_DPAD_RIGHT, GAMEPAD_DPAD_UP, GAMEPAD_TRIG_L,
    GAMEPAD_TRIG_R, GFX2D_BASE, GFX2D_CHAR_MAP_OFFSET, GFX2D_CHAR_MAP_SIZE, GFX2D_CONTROL_ENABLE,
    GFX2D_FONT_OFFSET, GFX2D_FRAME_INDEX_OFFSET, GFX2D_PALETTE_OFFSET, GFX2D_SCROLL_X_OFFSET,
    GFX2D_SCROLL_Y_OFFSET, INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER,
};
use riscv_shared::generate_sine_sample;

// `riscv` interrupt-number traits use `usize`, while the shared bus source ID is a `u32`.
const AUDIO_FIFO_LOW_WATER_INTERRUPT_NUMBER: usize =
    INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER as usize;
const INITIAL_FREQUENCY_DIV: u32 = 16;
const MAX_INTERRUPT_FILL_SAMPLES: u32 = 256;
const MIN_FREQUENCY_DIV: u32 = 1;
const MAX_FREQUENCY_DIV: u32 = 64;

const TILE_COLUMNS: u32 = 32;
const TILE_PIXELS: u32 = 8 * 8;
const TILE_ZERO: u8 = 0;
const TILE_ONE: u8 = 1;
const PALETTE_ORANGE: u32 = 0x00FF_A500;
const PALETTE_TEAL: u32 = 0x0000_8080;

static AUDIO_FREQUENCY_DIV: Mutex<Cell<u32>> = Mutex::new(Cell::new(INITIAL_FREQUENCY_DIV));
static AUDIO_SAMPLE_INDEX: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));

struct DemoState {
    frame_index: u32,
    scroll_x: u32,
    scroll_y: u32,
    frequency_div: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PocketExternalInterrupt {
    // This variant name intentionally exports the handler as `MachineExternal`
    // while its interrupt number maps to the audiosys FIFO low-water source ID.
    MachineExternal,
}

unsafe impl InterruptNumber for PocketExternalInterrupt {
    const MAX_INTERRUPT_NUMBER: usize = AUDIO_FIFO_LOW_WATER_INTERRUPT_NUMBER;

    fn number(self) -> usize {
        match self {
            Self::MachineExternal => AUDIO_FIFO_LOW_WATER_INTERRUPT_NUMBER,
        }
    }

    fn from_number(value: usize) -> Result<Self> {
        match value {
            AUDIO_FIFO_LOW_WATER_INTERRUPT_NUMBER => Ok(Self::MachineExternal),
            _ => Err(Error::InvalidVariant(value)),
        }
    }
}

unsafe impl ExternalInterruptNumber for PocketExternalInterrupt {}

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

fn set_audio_frequency_div(frequency_div: u32) {
    critical_section::with(|cs| {
        AUDIO_FREQUENCY_DIV.borrow(cs).set(frequency_div);
    });
}

fn reset_audio_sample_index() {
    critical_section::with(|cs| {
        AUDIO_SAMPLE_INDEX.borrow(cs).set(0);
    });
}

fn generate_fifo_sample_word() -> u32 {
    let (frequency_div, sample_index) = critical_section::with(|cs| {
        let frequency_div = AUDIO_FREQUENCY_DIV.borrow(cs).get();
        let sample_index_cell = AUDIO_SAMPLE_INDEX.borrow(cs);
        let sample_index = sample_index_cell.get();
        sample_index_cell.set(sample_index.wrapping_add(1));
        (frequency_div, sample_index)
    });
    let sample = generate_sine_sample(sample_index, frequency_div);
    let packed_sample = u16::from_ne_bytes(sample.to_ne_bytes());
    audiosys_fifo_pack_stereo_sample(packed_sample, packed_sample)
}

fn fill_audio_fifo(samples_to_write: u32, max_samples: u32) {
    let fill_count = samples_to_write.min(max_samples);
    for _ in 0..fill_count {
        write_u32(audiosys_fifo_sample_addr(), generate_fifo_sample_word());
    }
}

#[external_interrupt(PocketExternalInterrupt::MachineExternal)]
fn machine_external() {
    let claimed_source = read_u32(interrupt_ctrl_claim_addr());
    if claimed_source == 0 {
        return;
    }

    if claimed_source == INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER {
        let fifo_space = read_u32(audiosys_fifo_space_addr());
        fill_audio_fifo(fifo_space, MAX_INTERRUPT_FILL_SAMPLES);
    }

    write_u32(interrupt_ctrl_complete_addr(), claimed_source);
}

fn enable_audio_fifo_interrupts() {
    // Interrupt source IDs are 1-indexed, but the ENABLE register uses bit positions starting at 0.
    let enable_mask = 1u32 << (INTERRUPT_CTRL_SOURCE_AUDIOSYS_FIFO_LOW_WATER - 1);
    let current_enable = read_u32(interrupt_ctrl_enable_addr());
    write_u32(interrupt_ctrl_enable_addr(), current_enable | enable_mask);

    unsafe {
        // SAFETY: These CSR writes intentionally enable machine external interrupts
        // after the audiosys low-water interrupt source has been configured.
        riscv::register::mie::set_mext();
        riscv::register::mstatus::set_mie();
    }
}

fn initialize_demo() -> DemoState {
    let frequency_div = INITIAL_FREQUENCY_DIV;
    set_audio_frequency_div(frequency_div);
    reset_audio_sample_index();

    write_u32(audiosys_mode_addr(), AUDIOSYS_MODE_FIFO);
    write_u32(gfx2d_control_addr(), GFX2D_CONTROL_ENABLE);

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
        frequency_div,
    }
}

fn prime_audio_fifo() {
    let fifo_space = read_u32(audiosys_fifo_space_addr());
    fill_audio_fifo(fifo_space, fifo_space);
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

fn next_frequency_div(current: u32, input_state: u32) -> u32 {
    let lower_pitch = input_state & GAMEPAD_TRIG_L != 0;
    let higher_pitch = input_state & GAMEPAD_TRIG_R != 0;

    if lower_pitch == higher_pitch {
        current
    } else if lower_pitch {
        current.saturating_add(1).min(MAX_FREQUENCY_DIV)
    } else {
        current.saturating_sub(1).max(MIN_FREQUENCY_DIV)
    }
}

#[entry]
fn main() -> ! {
    let mut state = initialize_demo();
    prime_audio_fifo();
    enable_audio_fifo_interrupts();

    loop {
        state.frame_index = wait_for_next_frame(state.frame_index);

        let input_state = read_u32(gamepad_state_addr());
        let scroll_x_delta = button_delta(input_state, GAMEPAD_DPAD_LEFT, GAMEPAD_DPAD_RIGHT, 1);
        let scroll_y_delta = button_delta(input_state, GAMEPAD_DPAD_UP, GAMEPAD_DPAD_DOWN, 1);

        state.scroll_x = state.scroll_x.wrapping_add_signed(scroll_x_delta);
        state.scroll_y = state.scroll_y.wrapping_add_signed(scroll_y_delta);
        state.frequency_div = next_frequency_div(state.frequency_div, input_state);
        set_audio_frequency_div(state.frequency_div);

        write_u32(GFX2D_BASE + GFX2D_SCROLL_X_OFFSET, state.scroll_x);
        write_u32(GFX2D_BASE + GFX2D_SCROLL_Y_OFFSET, state.scroll_y);
    }
}
