use core::{future::Future, pin::Pin,};
use core::task::{Context, Poll};
use core::sync::atomic::{AtomicU64, Ordering};
use alloc::boxed::Box;

pub mod simple_executor;
pub mod keyboard;
pub mod executor;

/// The Task struct is a newtype wrapper around a pinned, heap-allocated, and dynamically dispatched future with the empty type () as output.
/// 
/// We require that the future associated with a task returns (). This means that tasks don’t return any result, they are just executed for their side effects.
/// The dyn keyword indicates that we store a trait object in the Box. This means that the methods on the future are dynamically dispatched, allowing different types of futures to be stored in the Task type.
/// As we learned in the section about pinning, the Pin<Box> type ensures that a value cannot be moved in memory by placing it on the heap and preventing the creation of &mut references to it. 
pub struct Task {
    // to uniquely name a task, which is required for waking a specific task.
    id: TaskId,
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    /// takes an arbitrary future with an output type of () 
    /// and pins it in memory through the Box::pin function
    /// 
    /// The 'static lifetime is required here because the returned Task can live for an arbitrary time, so the future needs to be valid for that time too.
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task { 
            id: TaskId::new(),
            future: Box::pin(future) 
        }
    }

    /// to allow the executor to poll the stored future
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        // 1. we use the Pin::as_mut method to convert the self.future field of type Pin<Box<T>>
        // 2. call poll on the converted self.future field and return the result.
        self.future.as_mut().poll(context)
    }
}

/// to fix the performance issues of polling
/// 
/// creating an executor with proper support for waker notifications is to give each task a unique ID. This is required because we need a way to specify which task should be woken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        // uses a static NEXT_ID variable of type AtomicU64 to ensure that each ID is assigned only once. 
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        // `fetch_add` atomically increases the value and returns the previous value in one atomic operation. 
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
