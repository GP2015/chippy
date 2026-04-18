use crate::config::{GPUConfig, RenderOccasion};
use crate::emulib::Limiter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

const CONDVAR_WAIT_TIMEOUT: Duration = Duration::from_millis(100);

pub struct Gpu {
    active: Arc<AtomicBool>,
    config: GPUConfig,
    framebuffer: Mutex<Vec<bool>>,
    render_queued: Mutex<bool>,
    render_queue_cvar: Condvar,
}

impl Gpu {
    pub fn try_new(active: Arc<AtomicBool>, config: GPUConfig) -> Option<Arc<Self>> {
        if config.render_occasion == RenderOccasion::Frequency && config.render_frequency <= 0.0 {
            eprintln!("The graphic render frequency must be greater than 0.");
            active.store(false, Ordering::Relaxed);
            return None;
        }

        let framebuffer_size = config.horizontal_resolution * config.vertical_resolution;

        Some(Arc::new(Self {
            active,
            config,
            framebuffer: Mutex::new(vec![false; framebuffer_size]),
            render_queued: Mutex::new(false),
            render_queue_cvar: Condvar::new(),
        }))
    }

    #[cfg(test)]
    pub fn new_default_wrapping(active: Arc<AtomicBool>) -> Arc<Self> {
        Self::try_new(
            active,
            GPUConfig {
                pixel_color_when_active: 0xFFFFFF,
                pixel_color_when_inactive: 0x000000,
                screen_border_color: 0x777777,
                horizontal_resolution: 64,
                vertical_resolution: 32,
                wrap_sprite_positions: true,
                wrap_sprite_pixels: true,
                render_occasion: RenderOccasion::Changes,
                render_frequency: 0.0,
            },
        )
        .unwrap()
    }

    // #[cfg(test)]
    // pub fn new_default_no_wrapping(active: Arc<AtomicBool>) -> Arc<Self> {
    //     Self::try_new(
    //         active,
    //         GPUConfig {
    //             pixel_color_when_active: 0xFFFFFF,
    //             pixel_color_when_inactive: 0x000000,
    //             screen_border_color: 0x777777,
    //             horizontal_resolution: 64,
    //             vertical_resolution: 32,
    //             wrap_sprite_positions: false,
    //             wrap_sprite_pixels: false,
    //             render_occasion: RenderOccasion::Changes,
    //             render_frequency: 0.0,
    //         },
    //     )
    //     .unwrap()
    // }

    pub fn should_render_separately(&self) -> bool {
        self.config.render_occasion == RenderOccasion::Frequency
    }

    pub fn run_separate_render(&self) {
        let mut limiter = Limiter::new(self.config.render_frequency, true);

        while self.active.load(Ordering::Relaxed) {
            limiter.wait_if_early();

            self.queue_render();
        }
    }

    pub fn get_screen_resolution(&self) -> (usize, usize) {
        (
            self.config.horizontal_resolution,
            self.config.vertical_resolution,
        )
    }

    pub fn get_active_color(&self) -> u32 {
        self.config.pixel_color_when_active
    }

    pub fn get_inactive_color(&self) -> u32 {
        self.config.pixel_color_when_inactive
    }

    pub fn get_border_color(&self) -> u32 {
        self.config.screen_border_color
    }

    pub fn get_framebuffer(&self) -> MutexGuard<'_, Vec<bool>> {
        self.framebuffer.lock().unwrap()
    }

    pub fn is_render_queued(&self) -> bool {
        *self.render_queued.lock().unwrap()
    }

    pub fn queue_render(&self) {
        *self.render_queued.lock().unwrap() = true;
    }

    pub fn dequeue_render(&self) {
        *self.render_queued.lock().unwrap() = false;
        self.render_queue_cvar.notify_all();
    }

    pub fn wait_for_render(&self) {
        let mut render_queued = self.render_queued.lock().unwrap();

        while *render_queued && self.active.load(Ordering::Relaxed) {
            (render_queued, _) = self
                .render_queue_cvar
                .wait_timeout(render_queued, CONDVAR_WAIT_TIMEOUT)
                .unwrap();
        }
    }

    pub fn clear_framebuffer(&self) {
        self.framebuffer.lock().unwrap().fill(false);

        if self.config.render_occasion == RenderOccasion::Changes {
            self.queue_render();
        }
    }

    pub fn draw_sprite(&self, sprite: &[u8], x_pos: u8, y_pos: u8) -> bool {
        debug_assert!(
            sprite.len() < 16,
            "Should not be draw a sprite larger than 16 bytes."
        );

        let mut x_pos = x_pos as usize;
        let mut y_pos = y_pos as usize;

        if self.config.wrap_sprite_positions {
            x_pos %= self.config.horizontal_resolution;
            y_pos %= self.config.vertical_resolution;
        } else if x_pos >= self.config.horizontal_resolution
            || y_pos >= self.config.vertical_resolution
        {
            return false;
        }

        let mut collided = false;
        let mut framebuffer = self.framebuffer.lock().unwrap();

        for (i, &pix) in sprite.iter().enumerate() {
            if self.draw_byte(&mut framebuffer, pix, x_pos, y_pos + i) {
                collided = true;
            }
        }

        if self.config.render_occasion == RenderOccasion::Changes {
            self.queue_render();
        }

        collided
    }

    fn draw_byte(
        &self,
        framebuffer: &mut MutexGuard<'_, Vec<bool>>,
        mut byte: u8,
        x_pos: usize,
        y_pos: usize,
    ) -> bool {
        let mut collided = false;

        for i in (0..8).rev() {
            let should_draw_bit = byte & 0x01 == 1;
            byte >>= 1;

            if !should_draw_bit {
                continue;
            }

            if let Some(true) = self.draw_pixel(framebuffer, x_pos + i, y_pos) {
                collided = true;
            }
        }

        collided
    }

    fn draw_pixel(
        &self,
        framebuffer: &mut MutexGuard<'_, Vec<bool>>,
        mut x_pos: usize,
        mut y_pos: usize,
    ) -> Option<bool> {
        let width = self.config.horizontal_resolution;
        let height = self.config.vertical_resolution;

        if self.config.wrap_sprite_pixels {
            x_pos %= width;
            y_pos %= height;
        } else if x_pos >= width || y_pos >= height {
            return None;
        }

        let index = y_pos * width + x_pos;

        let collision = framebuffer[index];
        framebuffer[index] ^= true;
        Some(collision)
    }
}
