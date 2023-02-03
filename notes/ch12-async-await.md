# async & await
## multitasking
**preemptive multitasking**: the OS controls can fully control the allowed execution time of task
- pros: guarantee that each task gets a fair share of CPU time, without the need to trust the tasks to cooperate.
- cons: 
  - each task requires its own stack => higher memory usage per task and often limits # of tasks in the system
  - the OS has to save the complete CPU register state on each task switch, even if the task only used a small subset of the registers.
- saving state (context switch): Since tasks are interrupted at arbitrary points in time, they might be in the middle of some calculations. In order to be able to resume them later, the operating system must backup the whole state of the task, including its call stack and the values of all CPU registers.

**thread/thread of execution**: a task with its own stack

=> by using a separate stack for each task, only the register contents need to be saved on a context switch (including the program counter and stack pointer.)

=> minimizes the performance overhead of context switch (often occur up to 100 times per sec).

**cooperative multitasking**: let each task run until it voluntarily gives up the control of the CPU.
- pros: the strong performance and memory benefits of cooperative multitasking make it a good approach for usage within a program, especially in combination with asynchronous operations.
- cons: an uncooperative task can potentially run for an unlimited amount of time;
- saving sate: Since tasks define their pause points themselves, they don’t need the operating system to save their state. Instead, they can save exactly the state they need for continuation before they pause themselves, which often results in better performance.

## async & await
for example:
```rust
async fn example(min_len: usize) -> String {
    let content = async_read_file("foo.txt").await;
    if content.len() < min_len {
        content + &async_read_file("bar.txt").await
    } else {
        content
    }
}
```

is equal to

```rust
fn example(min_len: usize) -> impl Future<Output = String> {
    async_read_file("foo.txt").then(move |content| {
        if content.len() < min_len {
            Either::Left(async_read_file("bar.txt").map(|s| content + &s))
        } else {
            Either::Right(future::ready(content))
        }
    })
}

```

Here we read the file `foo.txt` and then use the `then` combinator to chain a second future based on the file content. If the content length is smaller than the given min_len, we read a different `bar.txt` file and append it to `content` using the `map` combinator. Otherwise, we return only the content of `foo.txt`.

Using the .await operator, we can retrieve the value of a future without needing any closures or Either types.

**the state structures**
Behind the scenes, the compiler converts the body of the async function into a state machine, with each .await call representing a different state.

four states:
![](https://i.imgur.com/UGhwz9u.png)

To create a state machine on top of the structures representing the different states and containing the required variables, we can combine them into an enum:

In order to be able to continue from the last waiting state, the state machine must keep track of the current state internally. In addition, it must save all the variables that it needs to continue execution on the next `poll` call.

```rust
enum ExampleStateMachine {
    Start(StartState),
    WaitingOnFooTxt(WaitingOnFooTxtState),
    WaitingOnBarTxt(WaitingOnBarTxtState),
    End(EndState),
}
```

```rust
struct StartState {
    min_len: usize,
}

struct WaitingOnFooTxtState {
    min_len: usize,
    foo_txt_future: impl Future<Output = String>,
}

struct WaitingOnBarTxtState {
    content: String,
    bar_txt_future: impl Future<Output = String>,
}

struct EndState {}
```

**the state transformation**

![](https://i.imgur.com/xw9hkgk.png)

To implement the state transitions, the compiler generates an implementation of the Future trait based on the example function:

```rust
impl Future for ExampleStateMachine {
    type Output = String; // return type of `example`

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            match self { // TODO: handle pinning
                ExampleStateMachine::Start(state) => {…}
                ExampleStateMachine::WaitingOnFooTxt(state) => {…}
                ExampleStateMachine::WaitingOnBarTxt(state) => {…}
                ExampleStateMachine::End(state) => {…}
            }
        }
    }
}
```

the code for `Start` state:
```rust
// execute all the code from the body of the example function until the first .await.
ExampleStateMachine::Start(state) => {
    // from body of `example`
    let foo_txt_future = async_read_file("foo.txt");
    // `.await` operation
    //  To handle the .await operation, we change the state of the self state machine to WaitingOnFooTxt, 
    //  which includes the construction of the WaitingOnFooTxtState struct.
    let state = WaitingOnFooTxtState {
        min_len: state.min_len,
        foo_txt_future,
    };
    *self = ExampleStateMachine::WaitingOnFooTxt(state);
}
```

Since the match self {…} statement is executed in a loop, the execution jumps to the `WaitingOnFooTxt` arm next:
```rust
ExampleStateMachine::WaitingOnFooTxt(state) => {
    // we first call the poll function of the foo_txt_future.
    match state.foo_txt_future.poll(cx) {
        // If it is not ready, we exit the loop and return Poll::Pending
        Poll::Pending => return Poll::Pending,
        // When the foo_txt_future is ready, we assign the result to the content variable 
        // and continue to execute the code of the example function
        Poll::Ready(content) => {
            // from body of `example`
            if content.len() < state.min_len {
                let bar_txt_future = async_read_file("bar.txt");
                // `.await` operation
                let state = WaitingOnBarTxtState {
                    content,
                    bar_txt_future,
                };
                *self = ExampleStateMachine::WaitingOnBarTxt(state);
            } else {
                *self = ExampleStateMachine::End(EndState);
                return Poll::Ready(content);
            }
        }
    }
}
```

...



## pinning
**the problems with "self-referential structs"**
```rust
async fn pin_example() -> i32 {
    let array = [1, 2, 3];
    let element = &array[2];
    async_write_file("foo.txt", element.to_string()).await;
    *element
}
```

Since the function uses a single `await` operation, the resulting state machine has three states: start, end, and “waiting on write”. 
- The function takes no arguments, so the struct for the start state is empty. 
- Like before, the struct for the end state is empty because the function is finished at this point.
- The struct for the “waiting on write” state is more interesting: 
```rust
struct WaitingOnWriteState {
    array: [1, 2, 3],
    element: 0x1001c, // address of the last array element
}
```

We need to store both the `array` and `element` variables because `element` is required for the return value and `array` is referenced by `element`.

![](https://i.imgur.com/yPLPp1a.png)

**solution: forbid moving the structure**

As we saw above, the dangling pointer only occurs when we move the struct in memory. 

By completely forbidding move operations on self-referential structs, the problem can also be avoided. 
- pros: it can be implemented at the type system level without additional runtime costs. 
- cons: it puts the burden of dealing with move operations on possibly self-referential structs on the programmer

## executors and wakers
- **executor**:
The purpose of an executor is to allow spawning futures as independent tasks, typically through some sort of spawn method. The executor is then responsible for polling all futures until they are completed. 
- **waker**:
The idea behind the waker API is that a special Waker type is passed to each invocation of poll, wrapped in the Context type. This Waker type is created by the executor and can be used by the asynchronous task to signal its (partial) completion. 

## implementing cooperative multitasking

We see that futures and async/await fit the cooperative multitasking pattern perfectly; they just use some different terminology. In the following, we will therefore use the terms “task” and “future” interchangeably.

## async keyboard input
Currently, we handle the keyboard input directly in the interrupt handler. This is not a good idea for the long term because interrupt handlers should stay as short as possible as they might interrupt important work. 

Instead, interrupt handlers should only perform the minimal amount of work necessary (e.g., reading the keyboard scancode) and *leave the rest of the work* (e.g., interpreting the scancode) *to a background tas*k.

**scancode queue**
A common pattern for delegating work to a background task is to create some sort of queue. The interrupt handler pushes units of work to the queue, and the background task handles the work in the queue.

![](https://i.imgur.com/h8Po7tM.png)

- the interrupt handler only reads the scancode from the keyboard, pushes it to the queue, and then returns.
- The keyboard task sits on the other end of the queue and interprets and handles each scancode that is pushed to it

A simple implementation of that queue could be a mutex-protected VecDeque. However, using mutexes in interrupt handlers is not a good idea since it can easily lead to deadlocks. 

To prevent these problems, we need a queue implementation that does not require mutexes or allocations for its push operation. Such queues can be implemented by using lock-free atomic operations for pushing and popping elements.
=> crossbeam - `ArrayQueue`


