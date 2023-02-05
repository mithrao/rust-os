use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Waker, Context, Poll};
use crossbeam_queue::ArrayQueue;

pub struct Executor {
    // use a task_queue of task IDs and a BTreeMap named tasks that contains the actual Task instances.
    // The map is indexed by the TaskId to allow efficient continuation of a specific task.
    tasks: BTreeMap<TaskId, Task>,

    // ArrayQueue: wrapped into the Arc type that implements reference counting
    //             Reference counting makes it possible to share ownership of the value among multiple owners.
    //             It works by allocating the value on the heap and counting the number of active references to it. 
    //             When the number of active references reaches zero, the value is no longer needed and can be deallocated.
    // Arc<ArrayQueue>: it will be shared between the executor and wakers.
    //                  The wakers push the ID of the woken task to the queue. 
    //                  The executor sits on the receiving end of the queue, retrieves the woken tasks by their ID from the tasks map, and then runs them.
    task_queue: Arc<ArrayQueue<TaskId>>,
    
    // This map caches the Waker of a task after its creation. This has two reasons:
    // 1) it improves performance by reusing the same waker for multiple wake-ups of the same task instead of creating a new waker each time
    // 2) it ensures that reference-counted wakers are not deallocated inside interrupt handlers because it could lead to deadlocks
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            // The reason for using a fixed-size queue instead of an unbounded queue such as SegQueue is that interrupt handlers should not allocate on push to this queue
            // We choose a capacity of 100 for the task_queue, which should be more than enough for the foreseeable future.
            task_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }

    /// adds a given task to the tasks map 
    /// and immediately wakes it by pushing its ID to the task_queue
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }

    fn run_ready_tasks(&mut self) {
        // destructure `self` to avoid borrow checker errors
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        // Loop over all tasks in the task_queue, create a waker for each task, and then poll them
        while let Ok(task_id) = task_queue.pop() {
            // For each popped task ID, we retrieve a mutable reference to the corresponding task from the tasks map. 
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                // Since our ScancodeStream implementation registers wakers before checking whether a task needs to be put to sleep, it might happen that a wake-up occurs for a task that no longer exists.
                // In this case, we simply ignore the wake-up and continue with the next ID from the queue.
                None => continue, // task no longer exists
            };
            // To avoid the performance overhead of creating a waker on each poll, we use the waker_cache map to store the waker for each task after it has been created.
            let waker = waker_cache
                // `entry`+`or_insert_with`: to create a new waker if it doesn’t exist yet and then get a mutable reference to it
                .entry(task_id)
                // For creating a new waker, we clone the task_queue and pass it together with the task ID to the TaskWaker::new function (implementation shown below).
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cache waker
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    /// Since the function never returns, we use the ! return type to mark the function as diverging to the compiler.
    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            // We no longer poll tasks until they are woken again, but we still check the task_queue in a busy loop.
            // To fix this, we need to put the CPU to sleep if there is no more work to do.
            self.sleep_if_idel();
        }
    }

    fn sleep_if_idel(&self) {
        use x86_64::instructions::interrupts::{self, enable_and_hlt};
        // there is still a subtle race condition in this implementation. 
        // Since interrupts are asynchronous and can happen at any time, it is possible that an interrupt happens right between the is_empty check and the call to hlt

        // The answer is to disable interrupts on the CPU before the check and atomically enable them again together with the hlt instruction.
        // This way, all interrupts that happen in between are delayed after the hlt instruction so that no wake-ups are missed. 
        interrupts::disable();
        if self.task_queue.is_empty() {
            // <--- interrupt can happen here
            enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }
}


/// The job of the waker is to push the ID of the woken task to the task_queue of the executor. 
struct TaskWaker {
    task_id: TaskId,
    // Since the ownership of the task_queue is shared between the executor and wakers, we use the Arc wrapper type to implement shared reference-counted ownership
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    /// create the TaskWaker using the passed task_id and task_queue
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        // wrap the TaskWaker in an Arc and use the Waker::from implementation to convert it to a Waker.
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        self.task_queue.push(self.task_id).expect("task_queue full");
    }
}

/// In order to use our TaskWaker type for polling futures, we need to convert it to a Waker instance first.
impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}