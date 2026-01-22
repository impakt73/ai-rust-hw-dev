use cpu_sim::{VideoConfig, VideoFormat};
use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Duration;

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, Modifiers, WindowEvent as WinitWindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::{Window, WindowAttributes, WindowId};

pub struct VideoWindow {
    event_loop: EventLoop<()>,
    app: VideoWindowApp,
}

/// Internal application handler for winit events
struct VideoWindowApp {
    /// The actual window (created lazily on `resumed`)
    window: Option<Rc<Window>>,

    /// Softbuffer context and surface (created after window)
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,

    /// Desired window dimensions
    width: usize,
    height: usize,

    /// Frame buffer for softbuffer (0x00RRGGBB format)
    framebuffer: Vec<u32>,

    /// Event queue for communicating with main loop
    event_queue: VecDeque<WindowEvent>,

    /// Current modifier key states
    modifiers: Modifiers,

    /// Window title to set on creation
    pending_title: String,

    /// Whether the window has been closed
    closed: bool,
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

impl VideoWindowApp {
    fn new(width: usize, height: usize) -> Self {
        let framebuffer = vec![0x00000000u32; width * height];

        Self {
            window: None,
            context: None,
            surface: None,
            width,
            height,
            framebuffer,
            event_queue: VecDeque::new(),
            modifiers: Modifiers::default(),
            pending_title: "sim-view - No program loaded".to_string(),
            closed: false,
        }
    }

    /// Resize the softbuffer surface to match new dimensions
    fn resize_surface(&mut self, new_width: usize, new_height: usize) {
        if let Some(surface) = &mut self.surface {
            if let (Some(w), Some(h)) = (
                NonZeroU32::new(new_width as u32),
                NonZeroU32::new(new_height as u32),
            ) {
                if let Err(e) = surface.resize(w, h) {
                    log::error!("Failed to resize surface: {}", e);
                }
            }
        }
    }

    /// Present the framebuffer to the window
    fn present(&mut self) -> Result<(), String> {
        let surface = match &mut self.surface {
            Some(s) => s,
            None => return Err("Surface not initialized".to_string()),
        };

        let mut buffer = surface
            .buffer_mut()
            .map_err(|e| format!("Failed to get buffer: {}", e))?;

        // Get surface dimensions
        let surface_width = buffer.width().get() as usize;
        let surface_height = buffer.height().get() as usize;
        let surface_size = surface_width * surface_height;

        // Copy framebuffer to surface buffer, handling size mismatches
        let copy_len = self.framebuffer.len().min(surface_size);
        buffer[..copy_len].copy_from_slice(&self.framebuffer[..copy_len]);

        // Fill any remaining pixels with black if surface is larger
        if surface_size > copy_len {
            for pixel in &mut buffer[copy_len..] {
                *pixel = 0x00000000;
            }
        }

        buffer
            .present()
            .map_err(|e| format!("Failed to present buffer: {}", e))?;

        Ok(())
    }
}

impl ApplicationHandler for VideoWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return; // Already created
        }

        let window_attributes = WindowAttributes::default()
            .with_title(&self.pending_title)
            .with_inner_size(PhysicalSize::new(self.width as u32, self.height as u32))
            .with_resizable(true);

        match event_loop.create_window(window_attributes) {
            Ok(window) => {
                let window = Rc::new(window);

                // Create softbuffer context and surface
                let context = match Context::new(Rc::clone(&window)) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        log::error!("Failed to create softbuffer context: {}", e);
                        return;
                    }
                };

                let mut surface = match Surface::new(&context, Rc::clone(&window)) {
                    Ok(surf) => surf,
                    Err(e) => {
                        log::error!("Failed to create softbuffer surface: {}", e);
                        return;
                    }
                };

                // Initialize surface size
                if let (Some(w), Some(h)) = (
                    NonZeroU32::new(self.width as u32),
                    NonZeroU32::new(self.height as u32),
                ) {
                    if let Err(e) = surface.resize(w, h) {
                        log::error!("Failed to set initial surface size: {}", e);
                    }
                }

                self.window = Some(window);
                self.context = Some(context);
                self.surface = Some(surface);
            }
            Err(e) => {
                log::error!("Failed to create window: {}", e);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WinitWindowEvent,
    ) {
        match event {
            WinitWindowEvent::CloseRequested => {
                self.closed = true;
                self.event_queue.push_back(WindowEvent::Close);
                event_loop.exit();
            }
            WinitWindowEvent::Resized(size) => {
                let new_width = size.width as usize;
                let new_height = size.height as usize;
                if new_width > 0 && new_height > 0 {
                    self.resize_surface(new_width, new_height);
                }
            }
            WinitWindowEvent::RedrawRequested => {
                if let Err(e) = self.present() {
                    log::error!("Failed to present: {}", e);
                }
            }
            WinitWindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }
            WinitWindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                let ctrl = self.modifiers.state().control_key();
                let shift = self.modifiers.state().shift_key();
                let alt = self.modifiers.state().alt_key();
                let modifiers = KeyModifiers { ctrl, shift, alt };

                match logical_key {
                    WinitKey::Named(NamedKey::Escape) => {
                        self.event_queue
                            .push_back(WindowEvent::KeyPressed(Key::Escape, modifiers));
                    }
                    WinitKey::Named(NamedKey::Space) => {
                        self.event_queue
                            .push_back(WindowEvent::KeyPressed(Key::Space, modifiers));
                    }
                    WinitKey::Character(ref c) if c.eq_ignore_ascii_case("r") => {
                        self.event_queue
                            .push_back(WindowEvent::KeyPressed(Key::R, modifiers));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl VideoWindow {
    pub fn new(width: usize, height: usize) -> Result<Self, String> {
        let event_loop =
            EventLoop::new().map_err(|e| format!("Failed to create event loop: {}", e))?;

        let app = VideoWindowApp::new(width, height);

        Ok(VideoWindow { event_loop, app })
    }

    /// Process a video frame from the simulator controller
    /// This is called by the main viewer loop when a new frame is available
    pub fn process_video_frame(&mut self, data: &[u8], config: &VideoConfig) -> Result<(), String> {
        let new_width = config.width as usize;
        let new_height = config.height as usize;

        // Resize internal framebuffer and surface if dimensions changed
        if new_width != self.app.width || new_height != self.app.height {
            log::info!("Resizing window to {}x{}", new_width, new_height);
            self.app.width = new_width;
            self.app.height = new_height;
            self.app
                .framebuffer
                .resize(new_width * new_height, 0x00000000);

            // Resize the softbuffer surface
            self.app.resize_surface(new_width, new_height);

            // Request window resize if window exists
            if let Some(window) = &self.app.window {
                let _ = window
                    .request_inner_size(PhysicalSize::new(new_width as u32, new_height as u32));
            }
        }

        // Convert frame data to 0x00RRGGBB format for softbuffer
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

    /// Update window events (call once per frame in main loop)
    pub fn update_events(&mut self) -> Result<(), String> {
        // Pump events without blocking (timeout = 0)
        let status = self
            .event_loop
            .pump_app_events(Some(Duration::ZERO), &mut self.app);

        match status {
            PumpStatus::Exit(_) => {
                // Event loop requested exit
                if !self.app.closed {
                    self.app.closed = true;
                    self.app.event_queue.push_back(WindowEvent::Close);
                }
            }
            PumpStatus::Continue => {
                // Normal operation
            }
        }

        Ok(())
    }

    /// Update the window display (call once per frame in main loop)
    pub fn update_display(&mut self) -> Result<(), String> {
        // Request a redraw
        if let Some(window) = &self.app.window {
            window.request_redraw();
        }

        // Pump events to process the redraw request
        let status = self
            .event_loop
            .pump_app_events(Some(Duration::ZERO), &mut self.app);

        match status {
            PumpStatus::Exit(_) => {
                // Event loop requested exit
                if !self.app.closed {
                    self.app.closed = true;
                    self.app.event_queue.push_back(WindowEvent::Close);
                }
            }
            PumpStatus::Continue => {
                // Normal operation
            }
        }

        Ok(())
    }

    /// Convert RGBA8 (32-bit) to 0x00RRGGBB for softbuffer
    fn convert_rgba8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.app.width * self.app.height;
        if data.len() < pixel_count * 4 {
            return Err("Frame data too small for RGBA8 format".to_string());
        }

        for i in 0..pixel_count {
            let r = data[i * 4] as u32;
            let g = data[i * 4 + 1] as u32;
            let b = data[i * 4 + 2] as u32;
            // Note: softbuffer uses 0x00RRGGBB format (alpha is ignored)
            self.app.framebuffer[i] = (r << 16) | (g << 8) | b;
        }

        Ok(())
    }

    /// Convert RGB8 (24-bit) to 0x00RRGGBB for softbuffer
    fn convert_rgb8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.app.width * self.app.height;
        if data.len() < pixel_count * 3 {
            return Err("Frame data too small for RGB8 format".to_string());
        }

        for i in 0..pixel_count {
            let r = data[i * 3] as u32;
            let g = data[i * 3 + 1] as u32;
            let b = data[i * 3 + 2] as u32;
            self.app.framebuffer[i] = (r << 16) | (g << 8) | b;
        }

        Ok(())
    }

    /// Convert RGB565 (16-bit) to 0x00RRGGBB for softbuffer
    fn convert_rgb565(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.app.width * self.app.height;
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

            self.app.framebuffer[i] = (u32::from(r8) << 16) | (u32::from(g8) << 8) | u32::from(b8);
        }

        Ok(())
    }

    /// Convert R8 (8-bit grayscale) to 0x00RRGGBB for softbuffer
    fn convert_r8(&mut self, data: &[u8]) -> Result<(), String> {
        let pixel_count = self.app.width * self.app.height;
        if data.len() < pixel_count {
            return Err("Frame data too small for R8 format".to_string());
        }

        // Iterate over both framebuffer and data simultaneously
        for (fb_pixel, &gray) in self
            .app
            .framebuffer
            .iter_mut()
            .zip(data.iter())
            .take(pixel_count)
        {
            let gray32 = u32::from(gray);
            // Replicate gray value to R, G, B channels
            *fb_pixel = (gray32 << 16) | (gray32 << 8) | gray32;
        }

        Ok(())
    }

    /// Get pending events
    pub fn get_events(&mut self) -> Vec<WindowEvent> {
        self.app.event_queue.drain(..).collect()
    }

    /// Set window title
    pub fn set_title(&mut self, title: &str) {
        self.app.pending_title = title.to_string();
        if let Some(window) = &self.app.window {
            window.set_title(title);
        }
    }
}
