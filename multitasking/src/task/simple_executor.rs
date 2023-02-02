use super::Task;
use alloc::{collections::VecDeque};
use core::task::{Waker, RawWaker, RawWakerVTable, Context, Poll};

pub struct SimpleExecutor {
    // VecDeque: a vector that allows for push and pop operations on both ends
    // FIFO queue
    task_queue: VecDeque<Task>,
}

impl SimpleExecutor {
    pub fn new() -> SimpleExecutor {
        SimpleExecutor { task_queue: VecDeque::new(), }
    }

    /// insert new tasks through the spawn method at the end 
    /// and pop the next task for execution from the front
    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }

    pub fn run(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            let waker = dummy_waker();
            // 1. creates a Context type by wrapping a Waker instance returned by our dummy_waker function
            let mut context = Context::from_waker(&waker);
            // 2. invokes the Task::poll method with this context
            match task.poll(&mut context) {
                // 3.1 the task is finished and we can continue with the next task
                Poll::Ready(()) => {} // task done
                // 3.2 add it to the back of the queue again so that it will be polled again in a subsequent loop iteration.
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}

// In order to call the poll method, we need to create a Context type, which wraps a Waker type

/// RawWaker: requires the programmer to explicitly define a virtual method table (vtable) that specifies the functions that should be called when the RawWaker is cloned, woken, or dropped
/// The layout of this vtable is defined by the RawWakerVTable type.
/// Typically, the RawWaker is created for some heap-allocated struct that is wrapped into the Box or Arc type.
fn dummy_raw_waker() -> RawWaker {
    // takes a *const () pointer and does nothing
    fn no_op(_: *const ()) {}
    // takes a *const () pointer and returns a new RawWaker by calling dummy_raw_waker again
    // Since the RawWaker does nothing, it does not matter that we return a new RawWaker from clone instead of cloning it.
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(0 as *const(), vtable)
}

fn dummy_waker() -> Waker {
    // unsafe: undefined behavior can occur if the programmer does not uphold the documented requirements of RawWaker.
    unsafe {
        Waker::from_raw(dummy_raw_waker())
    }
}
