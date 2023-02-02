use core::{future::Future, pin::Pin,};
use core::task::{Context, Poll};
use alloc::boxed::Box;

pub mod simple_executor;

/// The Task struct is a newtype wrapper around a pinned, heap-allocated, and dynamically dispatched future with the empty type () as output.
/// 
/// We require that the future associated with a task returns (). This means that tasks don’t return any result, they are just executed for their side effects.
/// The dyn keyword indicates that we store a trait object in the Box. This means that the methods on the future are dynamically dispatched, allowing different types of futures to be stored in the Task type.
/// As we learned in the section about pinning, the Pin<Box> type ensures that a value cannot be moved in memory by placing it on the heap and preventing the creation of &mut references to it. 
pub struct Task {
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    /// takes an arbitrary future with an output type of () 
    /// and pins it in memory through the Box::pin function
    /// 
    /// The 'static lifetime is required here because the returned Task can live for an arbitrary time, so the future needs to be valid for that time too.
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task { future: Box::pin(future) }
    }

    /// to allow the executor to poll the stored future
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        // 1. we use the Pin::as_mut method to convert the self.future field of type Pin<Box<T>>
        // 2. call poll on the converted self.future field and return the result.
        self.future.as_mut().poll(context)
    }
}
