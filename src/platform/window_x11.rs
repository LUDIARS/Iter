/// X11 window implementation with Cairo surface.

use crate::core::types::{KeyEvent, MouseEvent};
use std::ffi::CString;
use std::ptr;
use x11::xlib;

pub struct WindowX11 {
    display: *mut xlib::Display,
    window: xlib::Window,
    width: i32,
    height: i32,
    wm_delete_window: xlib::Atom,
    should_close: bool,
    screen: i32,

    // Mouse state tracking
    button_pressed: [bool; 4],

    // Pending events for external consumption
    pending_mouse_events: Vec<MouseEvent>,
    pending_key_events: Vec<KeyEvent>,
}

impl WindowX11 {
    pub fn new() -> Self {
        Self {
            display: ptr::null_mut(),
            window: 0,
            width: 0,
            height: 0,
            wm_delete_window: 0,
            should_close: false,
            screen: 0,
            button_pressed: [false; 4],
            pending_mouse_events: Vec::new(),
            pending_key_events: Vec::new(),
        }
    }

    pub fn create(&mut self, width: i32, height: i32, title: &str) -> bool {
        unsafe {
            self.display = xlib::XOpenDisplay(ptr::null());
            if self.display.is_null() {
                eprintln!("Failed to open X11 display");
                return false;
            }

            self.screen = xlib::XDefaultScreen(self.display);
            let root = xlib::XRootWindow(self.display, self.screen);

            self.window = xlib::XCreateSimpleWindow(
                self.display,
                root,
                0,
                0,
                width as u32,
                height as u32,
                0,
                xlib::XBlackPixel(self.display, self.screen),
                xlib::XBlackPixel(self.display, self.screen),
            );

            self.width = width;
            self.height = height;

            // Set window title
            let title_c = CString::new(title).unwrap();
            xlib::XStoreName(self.display, self.window, title_c.as_ptr());

            // Select events
            xlib::XSelectInput(
                self.display,
                self.window,
                xlib::ExposureMask
                    | xlib::ButtonPressMask
                    | xlib::ButtonReleaseMask
                    | xlib::PointerMotionMask
                    | xlib::KeyPressMask
                    | xlib::KeyReleaseMask
                    | xlib::StructureNotifyMask,
            );

            // WM_DELETE_WINDOW protocol
            let wm_delete = CString::new("WM_DELETE_WINDOW").unwrap();
            self.wm_delete_window =
                xlib::XInternAtom(self.display, wm_delete.as_ptr(), xlib::False);
            xlib::XSetWMProtocols(
                self.display,
                self.window,
                &mut self.wm_delete_window as *mut _,
                1,
            );

            xlib::XMapWindow(self.display, self.window);
            xlib::XFlush(self.display);

            true
        }
    }

    /// Create a Cairo context for the current window using cairo-sys FFI.
    pub fn create_cairo_context(&self) -> Option<cairo::Context> {
        unsafe {
            let visual = xlib::XDefaultVisual(self.display, self.screen);

            let surface_ptr = cairo_sys::cairo_xlib_surface_create(
                self.display,
                self.window,
                visual,
                self.width,
                self.height,
            );

            if surface_ptr.is_null() {
                return None;
            }

            let cr_ptr = cairo_sys::cairo_create(surface_ptr);
            // Surface is now referenced by the context, release our ref
            cairo_sys::cairo_surface_destroy(surface_ptr);

            if cr_ptr.is_null() {
                return None;
            }

            // Wrap the raw pointer in cairo-rs Context (takes ownership)
            Some(cairo::Context::from_raw_full(cr_ptr))
        }
    }

    /// Process pending X11 events. Returns false if window should close.
    pub fn poll_events(&mut self) -> bool {
        if self.should_close {
            return false;
        }

        self.pending_mouse_events.clear();
        self.pending_key_events.clear();

        unsafe {
            while xlib::XPending(self.display) > 0 {
                let mut event: xlib::XEvent = std::mem::zeroed();
                xlib::XNextEvent(self.display, &mut event);

                match event.get_type() {
                    xlib::Expose => {}
                    xlib::ButtonPress => {
                        let btn = event.button;
                        let button = btn.button as u8;
                        if (button as usize) < self.button_pressed.len() {
                            self.button_pressed[button as usize] = true;
                        }

                        if button == 4 || button == 5 {
                            let scroll_y = if button == 4 { 1.0 } else { -1.0 };
                            self.pending_mouse_events.push(MouseEvent {
                                x: btn.x as f64,
                                y: btn.y as f64,
                                button: 0,
                                scroll_y,
                                pressed: false,
                                released: false,
                                dragging: false,
                            });
                        } else {
                            self.pending_mouse_events.push(MouseEvent {
                                x: btn.x as f64,
                                y: btn.y as f64,
                                button,
                                scroll_y: 0.0,
                                pressed: true,
                                released: false,
                                dragging: false,
                            });
                        }
                    }
                    xlib::ButtonRelease => {
                        let btn = event.button;
                        let button = btn.button as u8;
                        if (button as usize) < self.button_pressed.len() {
                            self.button_pressed[button as usize] = false;
                        }

                        if button < 4 {
                            self.pending_mouse_events.push(MouseEvent {
                                x: btn.x as f64,
                                y: btn.y as f64,
                                button,
                                scroll_y: 0.0,
                                pressed: false,
                                released: true,
                                dragging: false,
                            });
                        }
                    }
                    xlib::MotionNotify => {
                        let motion = event.motion;
                        let dragging = self.button_pressed.iter().any(|&b| b);
                        let active_button = self
                            .button_pressed
                            .iter()
                            .enumerate()
                            .find(|(_, &b)| b)
                            .map(|(i, _)| i as u8)
                            .unwrap_or(0);

                        self.pending_mouse_events.push(MouseEvent {
                            x: motion.x as f64,
                            y: motion.y as f64,
                            button: active_button,
                            scroll_y: 0.0,
                            pressed: false,
                            released: false,
                            dragging,
                        });
                    }
                    xlib::KeyPress | xlib::KeyRelease => {
                        let key = event.key;
                        self.pending_key_events.push(KeyEvent {
                            keycode: key.keycode,
                            pressed: event.get_type() == xlib::KeyPress,
                            ctrl: key.state & xlib::ControlMask != 0,
                            shift: key.state & xlib::ShiftMask != 0,
                            alt: key.state & xlib::Mod1Mask != 0,
                        });
                    }
                    xlib::ConfigureNotify => {
                        let configure = event.configure;
                        if configure.width != self.width || configure.height != self.height {
                            self.width = configure.width;
                            self.height = configure.height;
                        }
                    }
                    xlib::ClientMessage => {
                        let cm = event.client_message;
                        if cm.data.get_long(0) as xlib::Atom == self.wm_delete_window {
                            self.should_close = true;
                            return false;
                        }
                    }
                    _ => {}
                }
            }
        }

        !self.should_close
    }

    /// Drain pending mouse events.
    pub fn take_mouse_events(&mut self) -> Vec<MouseEvent> {
        std::mem::take(&mut self.pending_mouse_events)
    }

    /// Drain pending key events.
    pub fn take_key_events(&mut self) -> Vec<KeyEvent> {
        std::mem::take(&mut self.pending_key_events)
    }

    pub fn flush(&self) {
        unsafe {
            xlib::XFlush(self.display);
        }
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }
}

impl Drop for WindowX11 {
    fn drop(&mut self) {
        unsafe {
            if !self.display.is_null() {
                xlib::XDestroyWindow(self.display, self.window);
                xlib::XCloseDisplay(self.display);
            }
        }
    }
}
