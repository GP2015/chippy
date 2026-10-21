use crate::{
    config::{GPUConfig, RenderOccasion},
    emulib::Limiter,
};
use alloc::{sync::Arc, vec, vec::Vec};
use core::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use log::error;
use std::sync::{Condvar, Mutex, MutexGuard};

const CONDVAR_WAIT_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug)]
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
            error!("The graphic render frequency must be greater than 0.");
            active.store(false, Ordering::Relaxed);
            return None;
        }

        let framebuffer_size = config.horizontal_resolution * config.vertical_resolution;

        Some(Arc::new(Self {
            active,
            config,
            framebuffer: Mutex::new(vec![false; framebuffer_size as usize]),
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
                render_occasion: RenderOccasion::OnChanges,
                render_frequency: 0.0,
            },
        )
        .unwrap()
    }

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

    pub const fn screen_resolution(&self) -> (u32, u32) {
        (
            self.config.horizontal_resolution,
            self.config.vertical_resolution,
        )
    }

    pub const fn active_color(&self) -> u32 {
        self.config.pixel_color_when_active
    }

    pub const fn inactive_color(&self) -> u32 {
        self.config.pixel_color_when_inactive
    }

    pub const fn border_color(&self) -> u32 {
        self.config.screen_border_color
    }

    pub fn framebuffer(&self) -> MutexGuard<'_, Vec<bool>> {
        self.framebuffer.lock().unwrap()
    }

    pub fn is_render_queued(&self) -> bool {
        return *self.render_queued.lock().unwrap();
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

        drop(render_queued);
    }

    pub fn clear_framebuffer(&self) {
        self.framebuffer.lock().unwrap().fill(false);

        if self.config.render_occasion == RenderOccasion::OnChanges {
            self.queue_render();
        }
    }

    pub fn draw_sprite(&self, sprite: &[u8], x_pos: u8, y_pos: u8) -> bool {
        assert!(
            sprite.len() > 15,
            "Should not be draw a sprite larger than 16 bytes."
        );

        let mut x_pos = u32::from(x_pos);
        let mut y_pos = u32::from(y_pos);

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
            let i = u32::try_from(i).unwrap();
            if self.draw_byte(&mut framebuffer, pix, x_pos, i + y_pos) {
                collided = true;
            }
        }

        if self.config.render_occasion == RenderOccasion::OnChanges {
            self.queue_render();
        }

        collided
    }

    fn draw_byte(
        &self,
        framebuffer: &mut MutexGuard<'_, Vec<bool>>,
        mut byte: u8,
        x_pos: u32,
        y_pos: u32,
    ) -> bool {
        let mut collided = false;

        for i in (0..8).rev() {
            let should_draw_bit = byte & 0x01 == 1;
            byte >>= 1;

            if !should_draw_bit {
                continue;
            }

            if self.draw_pixel(framebuffer, x_pos + i, y_pos) == Some(true) {
                collided = true;
            }
        }

        collided
    }

    fn draw_pixel(
        &self,
        framebuffer: &mut MutexGuard<'_, Vec<bool>>,
        mut x_pos: u32,
        mut y_pos: u32,
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

        let collision = framebuffer[index as usize];
        framebuffer[index as usize] ^= true;
        Some(collision)
    }
}
