use super::*;
use std::ops::{Deref, DerefMut};

type CodeCallback = unsafe extern "C" fn(Engine, u64, u32, *mut c_void);

pub(super) struct CodeObserver<T> {
    engine: Engine,
    hooks: Vec<usize>,
    state: Box<T>,
}

impl<T> CodeObserver<T> {
    pub(super) fn with<R>(
        machine: &mut Machine,
        state: T,
        capture: impl FnOnce(&mut Machine, &mut Self) -> R,
    ) -> R {
        let mut observer = Self {
            engine: machine.engine,
            hooks: Vec::new(),
            state: Box::new(state),
        };
        capture(machine, &mut observer)
    }

    pub(super) fn observe(
        &mut self,
        callback: CodeCallback,
        addresses: impl IntoIterator<Item = u64>,
    ) {
        for address in addresses {
            let mut hook = 0;
            check(unsafe {
                uc_hook_add(
                    self.engine,
                    &mut hook,
                    4,
                    callback as *mut c_void,
                    ptr::from_mut(self.state.as_mut()).cast(),
                    address,
                    address,
                )
            });
            self.hooks.push(hook);
        }
    }
}

impl<T> Deref for CodeObserver<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.state
    }
}

impl<T> DerefMut for CodeObserver<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.state
    }
}

impl<T> Drop for CodeObserver<T> {
    fn drop(&mut self) {
        for &hook in &self.hooks {
            check(unsafe { uc_hook_del(self.engine, hook) });
        }
    }
}
