use cpu_sim::{VideoConfig, VideoFormat};
use minifb::{Key as MinifbKey, KeyRepeat, Scale, Window, WindowOptions};
use std::collections::VecDeque;

pub struct VideoWindow {
    window: Window,
    width: usize,
    height: usize,

    /// Frame buffer for minifb (ARGB8888 format)
    framebuffer: Vec<u32>,

    /// Event queue for communicating with main loop
    event_queue: VecDeque<WindowEvent>,
}

/// Window event types
pub enum WindowEvent {
    KeyPressed(Key, KeyModifiers),
    Close,
}

/// Key codes (simplified)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Escape,
    Space,
    R,
}

/// Keyboard modifiers
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    #[allow(dead_code)]
    pub shift: bool,
    #[allow(dead_code)]
    pub alt: bool,
}

impl VideoWindow {
    pub fn new(width: usize, height: usize) -> Result<Self, String> {
        let mut window = Window::new(
            "sim-view - No program loaded",
            width,
            height,
            WindowOptions {
                resize: true,
                scale: Scale::X1,
                ..WindowOptions::default()
            },
        )
        .map_err(|e| format!("Failed to create window: {}", e))?;

        // Set target FPS for minifb internal timing
        window.set_target_fps(60);

        // Create black framebuffer
        let framebuffer = vec![0xFF000000u32; width * height];

        Ok(VideoWindow {
            window,
            width,
            height,
            framebuffer,
            event_queue: VecDeque::new(),
        })
    }

    /// Process a video frame from the simulator controller
    /// This is called by the main viewer loop when a new frame is available
    pub fn process_video_frame(&mut self, data: &[u8], config: &VideoConfig) -> Result<(), String> {
        let new_width = config.width as usize;
        let new_height = config.height as usize;

        // Resize window if dimensions changed
        if new_width != self.width || new_height != self.height {
            log::info!("Resizing window to {}x{}", new_width, new_height);
            self.width = new_width;
            self.height = new_height;
            self.framebuffer.resize(new_width * new_height, 0xFF000000);
        }

        // Convert frame data to ARGB8888 format for minifb
        match config.format {
            VideoFormat::Rgba8 => {
                self.convert_rgba8(data)?;
            }
            VideoFormat::Rgb8 => {
                self.convert_rgb8(data)?;
            }
            VideoFormat::Rgb565 => {
                self.convert_rgb565(data)?;
            }
            VideoFormat::R8 => {
                self.convert_r8(data)?;
            }
        }

        Ok(())
    }

    /// Update the window display (call once per frame in main loop)
    pub fn update(&mut self) -> Result<(), String> {
        // Update minifb window with current framebuffer
        self.window
            .update_with_buffer(&self.framebuffer, self.width, self.height)
            .map_err(|e| format!("Failed to update window: {}", e))?;

        // Collect events
        self.collect_events();

        Ok(())
    }

    /// Convert RGBA8 (32-bit) to ARGB8888 for minifb
    fn convert_rgba8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count * 4 {
            return Err("Frame data too small for RGBA8 format".to_string());
        }

        for i in 0..pixel_count {
            let r = data[i * 4] as u32;
            let g = data[i * 4 + 1] as u32;
            let b = data[i * 4 + 2] as u32;
            let a = data[i * 4 + 3] as u32;

            // minifb format: 0xAARRGGBB
            self.framebuffer[i] = (a << 24) | (r << 16) | (g << 8) | b;
        }

        Ok(())
    }

    /// Convert RGB8 (24-bit) to ARGB8888 for minifb
    fn convert_rgb8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count * 3 {
            return Err("Frame data too small for RGB8 format".to_string());
        }

        for i in 0..pixel_count {
            let r = data[i * 3] as u32;
            let g = data[i * 3 + 1] as u32;
            let b = data[i * 3 + 2] as u32;

            // minifb format: 0xAARRGGBB (opaque)
            self.framebuffer[i] = 0xFF000000 | (r << 16) | (g << 8) | b;
        }

        Ok(())
    }

    /// Convert RGB565 (16-bit) to ARGB8888 for minifb
    fn convert_rgb565(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count * 2 {
            return Err("Frame data too small for RGB565 format".to_string());
        }

        for i in 0..pixel_count {
            let pixel = u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]);

            // Extract 5-6-5 components
            let r5 = ((pixel >> 11) & 0x1F) as u8;
            let g6 = ((pixel >> 5) & 0x3F) as u8;
            let b5 = (pixel & 0x1F) as u8;

            // Scale to 8-bit (preserve precision)
            let r8 = (r5 << 3) | (r5 >> 2);
            let g8 = (g6 << 2) | (g6 >> 4);
            let b8 = (b5 << 3) | (b5 >> 2);

            // minifb format: 0xAARRGGBB (opaque)
            self.framebuffer[i] =
                0xFF000000 | (u32::from(r8) << 16) | (u32::from(g8) << 8) | u32::from(b8);
        }

        Ok(())
    }

    /// Convert R8 (8-bit grayscale) to ARGB8888 for minifb
    fn convert_r8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.width * self.height;
        if data.len() < pixel_count {
            return Err("Frame data too small for R8 format".to_string());
        }

        // Iterate over both framebuffer and data simultaneously
        for (fb_pixel, &gray) in self
            .framebuffer
            .iter_mut()
            .zip(data.iter())
            .take(pixel_count)
        {
            let gray32 = u32::from(gray);
            // Replicate gray value to R, G, B channels
            *fb_pixel = 0xFF000000 | (gray32 << 16) | (gray32 << 8) | gray32;
        }

        Ok(())
    }

    /// Collect events from minifb window
    fn collect_events(&mut self) {
        // Check for window close
        if !self.window.is_open() {
            self.event_queue.push_back(WindowEvent::Close);
            return;
        }

        // Check for keyboard events
        if self.window.is_key_pressed(MinifbKey::Escape, KeyRepeat::No) {
            self.event_queue.push_back(WindowEvent::KeyPressed(
                Key::Escape,
                KeyModifiers::default(),
            ));
        }

        if self.window.is_key_pressed(MinifbKey::Space, KeyRepeat::No) {
            self.event_queue
                .push_back(WindowEvent::KeyPressed(Key::Space, KeyModifiers::default()));
        }

        // Check for Ctrl+R
        let ctrl_pressed = self.window.is_key_down(MinifbKey::LeftCtrl)
            || self.window.is_key_down(MinifbKey::RightCtrl);
        if ctrl_pressed && self.window.is_key_pressed(MinifbKey::R, KeyRepeat::No) {
            self.event_queue.push_back(WindowEvent::KeyPressed(
                Key::R,
                KeyModifiers {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
            ));
        }

        // NOTE: minifb doesn't directly support drag-and-drop
        // This would require platform-specific code or a different windowing library
        // For MVP, omit drag-and-drop and document as future enhancement
    }

    /// Get pending events
    pub fn get_events(&mut self) -> Vec<WindowEvent> {
        self.event_queue.drain(..).collect()
    }

    /// Check if window is still open
    pub fn is_open(&self) -> bool {
        self.window.is_open()
    }

    /// Close the window
    pub fn close(&mut self) {
        // minifb windows close automatically when dropped
        // This is a no-op but kept for API consistency
    }

    /// Set window title
    pub fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }
}
