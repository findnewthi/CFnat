use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<Inner>,
}

struct Inner {
    cancelled: AtomicBool,
    notifier: tokio::sync::Notify,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                cancelled: AtomicBool::new(false),
                notifier: tokio::sync::Notify::new(),
            }),
        }
    }

    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::AcqRel) {
            self.inner.notifier.notify_waiters();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub fn cancelled(&self) -> CancelledFuture {
        CancelledFuture {
            inner: self.inner.clone(),
            notified: false,
        }
    }
}

pub struct CancelledFuture {
    inner: Arc<Inner>,
    notified: bool,
}

impl Future for CancelledFuture {
    type Output = ();

    
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.inner.cancelled.load(Ordering::Acquire) {
            return Poll::Ready(());
        }
        
        if !self.notified {
            let waker = cx.waker().clone();
            let inner = self.inner.clone();
            tokio::spawn(async move {
                inner.notifier.notified().await;
                waker.wake();
            });
            self.notified = true;
        }
        
        Poll::Pending
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}