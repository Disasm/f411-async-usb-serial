use stm32f4xx_hal::stm32::{Interrupt, NVIC};
use cortex_m::interrupt::Mutex;
use core::task::{Waker, Context, Poll};
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};
use core::future::Future;
use core::pin::Pin;

pub struct InterruptObject {
    nr: Interrupt,
    waker: Mutex<Cell<Option<Waker>>>,
    taken: AtomicBool,
}

impl InterruptObject {
    pub const fn new(nr: Interrupt) -> Self {
        Self {
            nr,
            waker: Mutex::new(Cell::new(None)),
            taken: AtomicBool::new(false),
        }
    }

    pub fn handle_interrupt(&self) {
        NVIC::mask(self.nr);

        // We are already in the interrupt context, construct cs in a dirty way
        let cs = unsafe { core::mem::transmute(()) };
        if let Some(waker) = self.waker.borrow(&cs).take() {
            waker.wake();
        }
        cortex_m::asm::sev();
    }

    fn arm(&self, waker: Waker) {
        cortex_m::interrupt::free(|cs| {
            self.waker.borrow(cs).set(Some(waker));
        })
    }

    pub fn get_handle(&'static self) -> Option<InterruptHandle> {
        if self.taken.swap(false, Ordering::SeqCst) {
            None
        } else {
            Some(InterruptHandle { obj: self })
        }
    }
}

pub struct InterruptHandle {
    obj: &'static InterruptObject,
}

impl InterruptHandle {
    pub async fn wait(&mut self) {
        self.await
    }

    pub fn unpend(&self) {
        NVIC::unpend(self.obj.nr)
    }
}

impl Future for InterruptHandle {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let obj = self.get_mut().obj;
        if NVIC::is_pending(obj.nr) {
            Poll::Ready(())
        } else {
            obj.arm(cx.waker().clone());
            unsafe { NVIC::unmask(obj.nr) };
            Poll::Pending
        }
    }
}
