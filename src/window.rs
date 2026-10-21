use crate::{gpu::Gpu, input::InputManager};
use alloc::vec;
use log::error;
use softbuffer::{Buffer, Context, Surface};
use std::{
    cmp,
    num::NonZeroU32,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowButtons, WindowId},
};
use winit_input_helper::WinitInputHelper;

const WINDOW_TITLE: &str = "CHIP-8 Emulator";
const BASE_RESOLUTION_SCALAR: u32 = 20;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn get(self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub const fn set(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Position {
    pub index: u32,
    pub x: u32,
    pub y: u32,
    screen_width: u32,
}

impl Position {
    pub const fn from_coords(x: u32, y: u32, screen_width: u32) -> Self {
        Self {
            index: screen_width * y + x,
            x,
            y,
            screen_width,
        }
    }

    pub const fn from_index(index: u32, screen_width: u32) -> Self {
        Self {
            index,
            x: index % screen_width,
            y: index / screen_width,
            screen_width,
        }
    }

    const fn update_index(&mut self) {
        self.index = self.screen_width * self.y + self.x;
    }

    pub const fn scale(mut self, factor: u32) -> Self {
        self.screen_width *= factor;
        self.x *= factor;
        self.y *= factor;
        self.update_index();
        self
    }

    pub const fn add_padding(mut self, x_margin: u32, y_margin: u32) -> Self {
        self.screen_width += x_margin * 2;
        self.x += x_margin;
        self.y += y_margin;
        self.update_index();
        self
    }

    pub const fn screen_width(&self) -> u32 {
        self.screen_width
    }
}

pub struct WindowManager {
    active: Arc<AtomicBool>,
    gpu: Arc<Gpu>,
    input_manager: Arc<InputManager>,
    window: Option<Rc<Window>>,
    base_size: Size,
    size_factor: u32,
    window_size: Size,
    input: WinitInputHelper,
    context: Option<Context<Rc<Window>>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
}

impl WindowManager {
    pub fn new(active: Arc<AtomicBool>, gpu: Arc<Gpu>, input_manager: Arc<InputManager>) -> Self {
        let (base_width, base_height) = gpu.screen_resolution();

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

        let border_color = self.gpu.border_color();

        let (window_width, window_height) = self.window_size.get();
        let (base_width, base_height) = self.base_size.get();
        let size_factor = self.size_factor;

        let x_margin = (window_width - base_width * size_factor) / 2;
        let y_margin = (window_height - base_height * size_factor) / 2;

        let gpu_buffer = self.gpu.framebuffer();

        let mut render_buffer = match surface.buffer_mut() {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to retrieve the render buffer ({e}).");
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
            let pixel_u32 = u32::try_from(pixel).unwrap();
            let pos = Position::from_index(pixel_u32, base_width)
                .scale(size_factor)
                .add_padding(x_margin, y_margin);

            let size = Size::new(self.size_factor, self.size_factor);

            let color = if gpu_buffer[pixel] {
                self.gpu.active_color()
            } else {
                self.gpu.inactive_color()
            };

            Self::render_square(pos, size, color, &mut render_buffer);
        }

        drop(gpu_buffer);

        if let Err(e) = render_buffer.present() {
            error!("Failed to present the render buffer ({e}).");
            self.active.store(false, Ordering::Relaxed);
        }
    }

    fn render_square(
        pos: Position,
        size: Size,
        color: u32,
        buffer: &mut Buffer<'_, Rc<Window>, Rc<Window>>,
    ) {
        let pixel_row = vec![color; size.width as usize];

        for row in 0..size.height {
            let start_index = pos.index + row * pos.screen_width();
            buffer[start_index as usize..(start_index + size.width) as usize]
                .copy_from_slice(&pixel_row);
        }
    }

    fn update_size(&mut self, new_size: PhysicalSize<u32>) {
        self.window_size.set(new_size.width, new_size.height);

        self.size_factor = cmp::min(
            new_size.width / self.base_size.width,
            new_size.height / self.base_size.height,
        );

        let Some(surface) = self.surface.as_mut() else {
            return;
        };

        let Some(new_size_width_nz) = NonZeroU32::new(new_size.width) else {
            error!("Failed to convert window width into NonZeroU32.");
            self.active.store(false, Ordering::Relaxed);
            return;
        };

        let Some(new_size_height_nz) = NonZeroU32::new(new_size.height) else {
            error!("Failed to convert window height into NonZeroU32.");
            self.active.store(false, Ordering::Relaxed);
            return;
        };

        if let Err(e) = surface.resize(new_size_width_nz, new_size_height_nz) {
            error!("Failed to resize the softbuffer surface ({e}).");
            self.active.store(false, Ordering::Relaxed);
        }
    }
}

impl ApplicationHandler for WindowManager {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_size = PhysicalSize::new(self.window_size.width, self.window_size.height);

        let increment_size = PhysicalSize::new(self.base_size.width, self.base_size.height);

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

        let should_render = if let Some(new_size) = self.input.window_resized() {
            self.update_size(new_size);
            true
        } else {
            self.gpu.is_render_queued()
        };

        if should_render && let Some(window) = self.window.as_ref() {
            self.gpu.dequeue_render();
            window.request_redraw();
        }
    }
}
