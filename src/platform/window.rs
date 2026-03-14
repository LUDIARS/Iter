/// Cross-platform window implementation using winit + softbuffer.

use crate::core::types::{KeyEvent, MouseEvent};
use softbuffer::Surface;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window as WinitWindow, WindowId};

/// Callback that is invoked each frame with the window state.
/// Returns false to quit.
pub type FrameCallback = dyn FnMut(&mut WindowState) -> bool;

/// Mutable state exposed to the frame callback.
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub mouse_events: Vec<MouseEvent>,
    pub key_events: Vec<KeyEvent>,
    /// The pixel buffer (ARGB u32 per pixel, row-major).
    pub pixel_buffer: Vec<u32>,
}

struct App {
    window: Option<Rc<WinitWindow>>,
    surface: Option<Surface<Rc<WinitWindow>, Rc<WinitWindow>>>,
    state: WindowState,
    button_pressed: [bool; 4],
    cursor_pos: (f64, f64),
    frame_callback: Box<FrameCallback>,
    title: String,
    initial_width: u32,
    initial_height: u32,
}

impl App {
    fn new(
        title: &str,
        width: u32,
        height: u32,
        frame_callback: Box<FrameCallback>,
    ) -> Self {
        Self {
            window: None,
            surface: None,
            state: WindowState {
                width,
                height,
                mouse_events: Vec::new(),
                key_events: Vec::new(),
                pixel_buffer: vec![0; (width * height) as usize],
            },
            button_pressed: [false; 4],
            cursor_pos: (0.0, 0.0),
            frame_callback,
            title: title.to_string(),
            initial_width: width,
            initial_height: height,
        }
    }

    fn present_buffer(&mut self) {
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let w = self.state.width;
        let h = self.state.height;
        let Some(nz_w) = NonZeroU32::new(w) else {
            return;
        };
        let Some(nz_h) = NonZeroU32::new(h) else {
            return;
        };
        surface.resize(nz_w, nz_h).ok();
        if let Ok(mut buf) = surface.buffer_mut() {
            let len = (w * h) as usize;
            let src = &self.state.pixel_buffer;
            if src.len() >= len {
                buf[..len].copy_from_slice(&src[..len]);
            }
            buf.present().ok();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WinitWindow::default_attributes()
            .with_title(&self.title)
            .with_inner_size(LogicalSize::new(self.initial_width, self.initial_height));
        let window = Rc::new(event_loop.create_window(attrs).expect("Failed to create window"));
        let context = softbuffer::Context::new(window.clone()).expect("Failed to create softbuffer context");
        let surface = Surface::new(&context, window.clone())
            .expect("Failed to create softbuffer surface");
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                let w = size.width.max(1);
                let h = size.height.max(1);
                self.state.width = w;
                self.state.height = h;
                self.state.pixel_buffer.resize((w * h) as usize, 0);
            }
            WindowEvent::CursorMoved {
                position: PhysicalPosition { x, y },
                ..
            } => {
                self.cursor_pos = (x, y);
                let dragging = self.button_pressed.iter().any(|&b| b);
                let active_button = self
                    .button_pressed
                    .iter()
                    .enumerate()
                    .find(|(_, &b)| b)
                    .map(|(i, _)| i as u8)
                    .unwrap_or(0);
                self.state.mouse_events.push(MouseEvent {
                    x,
                    y,
                    button: active_button,
                    scroll_y: 0.0,
                    pressed: false,
                    released: false,
                    dragging,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let btn: u8 = match button {
                    MouseButton::Left => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Right => 3,
                    _ => 0,
                };
                let pressed = state == ElementState::Pressed;
                if (btn as usize) < self.button_pressed.len() {
                    self.button_pressed[btn as usize] = pressed;
                }
                self.state.mouse_events.push(MouseEvent {
                    x: self.cursor_pos.0,
                    y: self.cursor_pos.1,
                    button: btn,
                    scroll_y: 0.0,
                    pressed,
                    released: !pressed,
                    dragging: false,
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(PhysicalPosition { y, .. }) => y / 30.0,
                };
                self.state.mouse_events.push(MouseEvent {
                    x: self.cursor_pos.0,
                    y: self.cursor_pos.1,
                    button: 0,
                    scroll_y,
                    pressed: false,
                    released: false,
                    dragging: false,
                });
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                let keycode = match event.physical_key {
                    PhysicalKey::Code(code) => winit_keycode_to_x11(code),
                    _ => 0,
                };
                // Approximate modifier detection from the key event
                // We track modifiers via ModifiersChanged below
                self.state.key_events.push(KeyEvent {
                    keycode,
                    pressed,
                    ctrl: false,
                    shift: false,
                    alt: false,
                });
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                // Apply modifiers to the most recent key event if any
                let state = modifiers.state();
                if let Some(last) = self.state.key_events.last_mut() {
                    last.ctrl = state.control_key();
                    last.shift = state.shift_key();
                    last.alt = state.alt_key();
                }
            }
            WindowEvent::RedrawRequested => {
                // Run frame callback
                let should_continue = (self.frame_callback)(&mut self.state);
                if !should_continue {
                    event_loop.exit();
                    return;
                }
                // Present the pixel buffer
                self.present_buffer();
                // Clear events
                self.state.mouse_events.clear();
                self.state.key_events.clear();
                // Request next frame
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

/// Map winit KeyCode to X11-style keycodes for compatibility with existing code.
fn winit_keycode_to_x11(code: KeyCode) -> u32 {
    match code {
        KeyCode::Escape => 9,
        KeyCode::Tab => 23,
        KeyCode::KeyQ => 24,
        KeyCode::Space => 65,
        KeyCode::Enter => 36,
        KeyCode::ArrowUp => 111,
        KeyCode::ArrowDown => 116,
        KeyCode::ArrowLeft => 113,
        KeyCode::ArrowRight => 114,
        _ => 0,
    }
}

/// Run the application event loop. This function does not return until the window is closed.
pub fn run_window(
    title: &str,
    width: u32,
    height: u32,
    frame_callback: impl FnMut(&mut WindowState) -> bool + 'static,
) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(title, width, height, Box::new(frame_callback));
    event_loop.run_app(&mut app).ok();
}
