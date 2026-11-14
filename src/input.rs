use crate::config::InputConfig;
use alloc::sync::Arc;
use core::{
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
    time::Duration,
};
use parking_lot::{Condvar, Mutex};
#[cfg(test)]
use winit::keyboard::Key;
#[cfg(test)]
use winit::keyboard::SmolStr;
use winit_input_helper::WinitInputHelper;

const NUMBER_OF_INPUTS: u8 = 16;
const CONDVAR_WAIT_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(PartialEq, Eq)]
enum NewestKeyState {
    Finished,
    Requested,
    Held,
    Sent,
}

pub struct InputManager {
    active: Arc<AtomicBool>,
    config: InputConfig,
    key_states: Mutex<[bool; 16]>,
    newest_key_state: Mutex<NewestKeyState>,
    newest_key: AtomicU8,
    newest_key_cvar: Condvar,
}

impl InputManager {
    pub fn try_new(active: Arc<AtomicBool>, config: InputConfig) -> Arc<Self> {
        Arc::new(Self {
            active,
            config,
            key_states: Mutex::new([false; 16]),
            newest_key_state: Mutex::new(NewestKeyState::Finished),
            newest_key: AtomicU8::new(0),
            newest_key_cvar: Condvar::new(),
        })
    }

    #[cfg(test)]
    pub fn new_default(active: Arc<AtomicBool>) -> Arc<Self> {
        Self::try_new(
            active,
            InputConfig {
                key_bindings: [
                    Key::Character(SmolStr::new("1")),
                    Key::Character(SmolStr::new("2")),
                    Key::Character(SmolStr::new("3")),
                    Key::Character(SmolStr::new("q")),
                    Key::Character(SmolStr::new("w")),
                    Key::Character(SmolStr::new("e")),
                    Key::Character(SmolStr::new("a")),
                    Key::Character(SmolStr::new("s")),
                    Key::Character(SmolStr::new("d")),
                    Key::Character(SmolStr::new("x")),
                    Key::Character(SmolStr::new("z")),
                    Key::Character(SmolStr::new("c")),
                    Key::Character(SmolStr::new("4")),
                    Key::Character(SmolStr::new("r")),
                    Key::Character(SmolStr::new("f")),
                    Key::Character(SmolStr::new("v")),
                ],
            },
        )
    }

    pub fn update_input(&self, input: &WinitInputHelper) {
        let mut key_states = self.key_states.lock();
        let mut newest_key_state = self.newest_key_state.lock();

        for i in 0..NUMBER_OF_INPUTS {
            if input.key_pressed_logical(self.config.key_bindings[i as usize].as_ref()) {
                key_states[i as usize] = true;

                if *newest_key_state == NewestKeyState::Requested {
                    self.newest_key.store(i, Ordering::Release);
                    *newest_key_state = NewestKeyState::Held;
                }
            } else if input.key_released_logical(self.config.key_bindings[i as usize].as_ref()) {
                key_states[i as usize] = false;

                if *newest_key_state == NewestKeyState::Held {
                    *newest_key_state = NewestKeyState::Sent;
                    self.newest_key_cvar.notify_all();
                }
            }
        }

        drop(key_states);
        drop(newest_key_state);

        self.newest_key_cvar.notify_all();
    }

    pub fn get_key_state(&self, key_index: u8) -> bool {
        debug_assert!(
            key_index <= 0xF,
            "Should not be possible to read non-existent key_states."
        );

        self.key_states.lock()[key_index as usize]
    }

    pub fn get_next_key_press(&self) -> u8 {
        let mut newest_key_state = self.newest_key_state.lock();

        while *newest_key_state != NewestKeyState::Finished && self.active.load(Ordering::Relaxed) {
            self.newest_key_cvar
                .wait_for(&mut newest_key_state, CONDVAR_WAIT_TIMEOUT);
        }

        *newest_key_state = NewestKeyState::Requested;
        self.newest_key_cvar.notify_all();

        while *newest_key_state != NewestKeyState::Sent && self.active.load(Ordering::Relaxed) {
            self.newest_key_cvar
                .wait_for(&mut newest_key_state, CONDVAR_WAIT_TIMEOUT);
        }

        *newest_key_state = NewestKeyState::Finished;
        drop(newest_key_state);
        self.newest_key_cvar.notify_all();

        self.newest_key.load(Ordering::Acquire)
    }
}
