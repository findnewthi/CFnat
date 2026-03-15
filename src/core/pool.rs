use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Semaphore;

pub struct ConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
}

impl ConcurrencyLimiter {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
        }
    }

    pub async fn acquire(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.semaphore.clone().acquire_owned().await.unwrap()
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }
}

pub static GLOBAL_LIMITER: OnceLock<ConcurrencyLimiter> = OnceLock::new();

pub fn init_global_limiter(max_concurrent: usize) {
    let _ = GLOBAL_LIMITER.set(ConcurrencyLimiter::new(max_concurrent));
}
