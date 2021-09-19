use crate::{Rpc, Lockable, LockGuard, lock};
use async_trait::async_trait;
use std::time::Duration;

pub struct ByteStreamPipe<T: Rpc> {
    rpc: T,
    addr: String,
}

#[async_trait]
impl<T: Rpc> Lockable<T> for ByteStreamPipe<T> {
    async fn lock(&mut self, timeout: Duration) -> crate::Result<LockGuard<T>> {
        lock(&mut self.rpc, &self.addr, timeout).await
    }
}

impl<T: Rpc> ByteStreamPipe<T> {
}
