use std::future::Future;
use crate::*;

#[derive(Clone)]
pub struct WasmExecutor;

impl<Fut> hyper::rt::Executor<Fut> for WasmExecutor
    where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        spawn_local(async move {fut.await;});
    }
}

unsafe impl Send for WasmExecutor {}
unsafe impl Sync for WasmExecutor {}
