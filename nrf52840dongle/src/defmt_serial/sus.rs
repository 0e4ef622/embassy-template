//! Suspiciously unsafe things

pub const fn fut_size<T, F, Fut>(_: F) -> usize
where
    F: Fn(T) -> Fut + Copy,
    Fut: Future + 'static,
{
    size_of::<Fut>()
}

pub const fn fut_align<T, F, Fut>(_: F) -> usize
where
    F: Fn(T) -> Fut + Copy,
    Fut: Future + 'static,
{
    align_of::<Fut>()
}

macro_rules! declare_static_future {
    ($static_name:ident = $fun_name:ident; $mod_name:ident) => {
        mod $mod_name {
            use core::cell::UnsafeCell;
            use core::mem::MaybeUninit;
            use core::sync::atomic::{AtomicBool, Ordering};
            use core::future::Future;
            use core::pin::Pin;

            pub struct StaticFuture {
                is_init: AtomicBool,
                fut: UnsafeCell<MaybeUninit<(
                    [u8; $crate::defmt_serial::sus::fut_size(super::$fun_name)],
                    embassy_executor::_export::Align<{$crate::defmt_serial::sus::fut_align(super::$fun_name)}
                >)>>,
            }

            impl StaticFuture {
                pub const fn new() -> Self {
                    Self {
                        is_init: AtomicBool::new(false),
                        fut: UnsafeCell::new(MaybeUninit::uninit()),
                    }
                }
                pub unsafe fn init<Fut>(&self, f: Fut) {
                    unsafe {
                        self.fut.get().cast::<Fut>().write(f);
                        self.is_init.store(true, Ordering::Release);
                    }
                }
                pub unsafe fn get(&self) -> Option<Pin<&'static mut impl Future>> {
                    if self.is_init.load(Ordering::Acquire) {
                        unsafe {
                            Some(Pin::static_mut(self._get(super::$fun_name)))
                        }
                    } else {
                        None
                    }
                }
                unsafe fn _get<'a, Args, F, Fut>(&self, _: F) -> &'a mut Fut
                where
                    F: embassy_executor::_export::TaskFn<Args, Fut = Fut>,
                    Fut: Future + 'static,
                {
                    unsafe { &mut *self.fut.get().cast() }
                }
            }

            unsafe impl Sync for StaticFuture {}
        }

        static $static_name: $mod_name::StaticFuture = $mod_name::StaticFuture::new();
    }
}
