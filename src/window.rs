use crate::gpu::Gpu;
use crate::input::InputManager;
use softbuffer::{Buffer, Context, Surface};
use std::cmp;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowButtons, WindowId};
use winit_input_helper::WinitInputHelper;

const WINDOW_TITLE: &str = "CHIP-8 Emulator";
const BASE_RESOLUTION_SCALAR: usize = 20;

#[derive(Clone, Copy)]
struct Size {
    pub width: usize,
    pub height: usize,
}

impl Size {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub fn get(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    pub fn set(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }
}

#[derive(Clone, Copy)]
struct Position {
    pub index: usize,
    pub x: usize,
    pub y: usize,
    screen_width: usize,
}

impl Position {
    pub fn from_coords(x: usize, y: usize, screen_width: usize) -> Self {
        Self {
            index: screen_width * y + x,
            x,
            y,
            screen_width,
        }
    }

    pub fn from_index(index: usize, screen_width: usize) -> Self {
        Self {
            index,
            x: index % screen_width,
            y: index / screen_width,
            screen_width,
        }
    }

    fn update_index(&mut self) {
        self.index = self.screen_width * self.y + self.x;
    }

    pub fn scale(mut self, factor: usize) -> Self {
        self.screen_width *= factor;
        self.x *= factor;
        self.y *= factor;
        self.update_index();
        self
    }

    pub fn add_padding(mut self, x_margin: usize, y_margin: usize) -> Self {
        self.screen_width += x_margin * 2;
        self.x += x_margin;
        self.y += y_margin;
        self.update_index();
        self
    }

    pub fn get_screen_width(&self) -> usize {
        self.screen_width
    }
}

pub struct WindowManager {
    active: Arc<AtomicBool>,
    gpu: Arc<Gpu>,
    input_manager: Arc<InputManager>,
    window: Option<Rc<Window>>,
    base_size: Size,
    size_factor: usize,
    window_size: Size,
    input: WinitInputHelper,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
}

impl WindowManager {
    pub fn new(active: Arc<AtomicBool>, gpu: Arc<Gpu>, input_manager: Arc<InputManager>) -> Self {
        let (base_width, base_height) = gpu.get_screen_resolution();

        let base_size = Size::new(base_width, base_height);

        let window_size = Size::new(
            base_width.saturating_mul(BASE_RESOLUTION_SCALAR),
            base_height.saturating_mul(BASE_RESOLUTION_SCALAR),
        );

        Self {
            active,
            gpu,
            input_manager,
            window: None,
            base_size,
            window_size,
            size_factor: BASE_RESOLUTION_SCALAR,
            input: WinitInputHelper::new(),
            context: None,
            surface: None,
        }
    }

    fn render(&mut self) {
        let Some(surface) = self.surface.as_mut() else {
            return;
        };

        let border_color = self.gpu.get_border_color();

        let (window_width, window_height) = self.window_size.get();
        let (base_width, base_height) = self.base_size.get();
        let size_factor = self.size_factor;

        let x_margin = (window_width - base_width * size_factor) / 2;
        let y_margin = (window_height - base_height * size_factor) / 2;

        let gpu_buffer = self.gpu.get_framebuffer();

        let mut render_buffer = match surface.buffer_mut() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Failed to retrieve the render buffer ({e}).");
                self.active.store(false, Ordering::Relaxed);
                return;
            }
        };

        if x_margin > 0 {
            Self::render_square(
                Position::from_coords(0, 0, window_width),
                Size::new(x_margin, window_height),
                border_color,
                &mut render_buffer,
            );

            Self::render_square(
                Position::from_coords(window_width - x_margin, 0, window_width),
                Size::new(x_margin, window_height),
                border_color,
                &mut render_buffer,
            );
        }

        if y_margin > 0 {
            Self::render_square(
                Position::from_coords(x_margin, 0, window_width),
                Size::new(window_width - (x_margin * 2), y_margin),
                border_color,
                &mut render_buffer,
            );

            Self::render_square(
                Position::from_coords(x_margin, window_height - y_margin, window_width),
                Size::new(window_width - (x_margin * 2), y_margin),
                border_color,
                &mut render_buffer,
            );
        }

        for pixel in 0..gpu_buffer.len() {
            let pos = Position::from_index(pixel, base_width)
                .scale(size_factor)
                .add_padding(x_margin, y_margin);

            let size = Size::new(self.size_factor, self.size_factor);

            let color = if gpu_buffer[pixel] {
                self.gpu.get_active_color()
            } else {
                self.gpu.get_inactive_color()
            };

            Self::render_square(pos, size, color, &mut render_buffer);
        }

        if let Err(e) = render_buffer.present() {
            eprintln!("Failed to present the render buffer ({e}).");
            self.active.store(false, Ordering::Relaxed);
        }
    }

    fn render_square(
        pos: Position,
        size: Size,
        color: u32,
        buffer: &mut Buffer<'_, Rc<Window>, Rc<Window>>,
    ) {
        let pixel_row = vec![color; size.width];

        for row in 0..size.height {
            let start_index = pos.index + row * pos.get_screen_width();
            buffer[start_index..start_index + size.width].copy_from_slice(&pixel_row);
        }
    }

    fn update_size(&mut self, new_size: PhysicalSize<u32>) {
        self.window_size
            .set(new_size.width as usize, new_size.height as usize);

        self.size_factor = cmp::min(
            new_size.width as usize / self.base_size.width,
            new_size.height as usize / self.base_size.height,
        );

        let Some(surface) = self.surface.as_mut() else {
            return;
        };

        let Some(new_size_width_nz) = NonZeroU32::new(new_size.width) else {
            eprintln!("Failed to convert window width into NonZeroU32.");
            self.active.store(false, Ordering::Relaxed);
            return;
        };

        let Some(new_size_height_nz) = NonZeroU32::new(new_size.height) else {
            eprintln!("Failed to convert window height into NonZeroU32.");
            self.active.store(false, Ordering::Relaxed);
            return;
        };

        if let Err(e) = surface.resize(new_size_width_nz, new_size_height_nz) {
            eprintln!("Failed to resize the softbuffer surface ({e}).");
            self.active.store(false, Ordering::Relaxed);
        }
    }
}

impl ApplicationHandler for WindowManager {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_size = PhysicalSize::new(
            self.window_size.width as u32,
            self.window_size.height as u32,
        );

        let increment_size =
            PhysicalSize::new(self.base_size.width as u32, self.base_size.height as u32);

        let attributes = Window::default_attributes()
            .with_inner_size(window_size)
            .with_title(WINDOW_TITLE)
            .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
            .with_resize_increments(increment_size);

        let window = Rc::new(event_loop.create_window(attributes).unwrap());
        let context = Context::new(window.clone()).unwrap();
        let surface = Surface::new(&context, window.clone()).unwrap();

        self.update_size(window_size);

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if self.input.process_window_event(&event) {
            self.render();
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        self.input.process_device_event(&event);
    }

    fn new_events(&mut self, _: &ActiveEventLoop, _: StartCause) {
        self.input.step();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.active.load(Ordering::Relaxed) {
            event_loop.exit();
            return;
        }

        self.input.end_step();

        if self.input.close_requested() || self.input.destroyed() {
            self.active.store(false, Ordering::Relaxed);
            event_loop.exit();
            return;
        }

        self.input_manager.update_input(&self.input);

        if let Some(new_size) = self.input.window_resized() {
            self.update_size(new_size);
            self.render();
        }

        let mut should_render = false;

        if let Some(new_size) = self.input.window_resized() {
            self.update_size(new_size);
            should_render = true;
        }

        if self.gpu.is_render_queued() {
            should_render = true;
        }

        if should_render && let Some(window) = self.window.as_ref() {
            self.gpu.dequeue_render();
            window.request_redraw();
        }
    }
}
