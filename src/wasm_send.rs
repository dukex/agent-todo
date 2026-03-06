use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Wraps a future to implement Send.
/// Safe in WASM because it is always single-threaded.
pub struct WasmSend<F>(pub F);

unsafe impl<F> Send for WasmSend<F> {}

impl<F: Future> Future for WasmSend<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { self.map_unchecked_mut(|s| &mut s.0) }.poll(cx)
    }
}
