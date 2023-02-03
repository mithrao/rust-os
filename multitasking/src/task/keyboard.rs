use conquer_once::{spin::OnceCell};
use crossbeam_queue::ArrayQueue;
use crate::{println, print};
use core::{pin::Pin, task::{Poll, Context}};
use futures_util::stream::{Stream, StreamExt};
use futures_util::task::AtomicWaker;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};


static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
// Like the Futures::poll method, the Stream::poll_next method requires the asynchronous task to notify the executor when it becomes ready after Poll::Pending is returned.
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the keyboard interrupt handler
///
/// Must not block or allocate.
/// Since this function should not be callable from our main.rs, we use the pub(crate) visibility to make it only available to our lib.rs.
pub(crate) fn add_scancode(scancode: u8) {
    // use OnceCell::try_get to get a reference to the initialized queue.
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        // the ArrayQueue::push method requires only a &self reference makes it very simple to call the method on the static queue.
        // The ArrayQueue type performs all the necessary synchronization itself, so we don’t need a mutex wrapper here.
        if let Err(_) = queue.push(scancode) {
            // In case the queue is full, we print a warning too.
            // we call wake only after pushing to the queue because otherwise the task might be woken too early while the queue is still empty
            println!("WARNING: scancode queue full; dropping keyboard input");
        } else {
            // add a call to WAKER.wake() if the push to the scancode queue succeeds.
            WAKER.wake();
        }
    } else {
        // If the queue is not initialized yet, we ignore the keyboard scancode and print a warning. 
        println!("WARNING: scancode queue uninitialized");
    }
}


/// To initialize the SCANCODE_QUEUE and read the scancodes from the queue in an asynchronous way
pub struct ScancodeStream {
    // The purpose of the _private field is to prevent construction of the struct from outside of the module.
    // This makes the new function the only way to construct the type. 
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        // initialize the SCANCODE_QUEUE static
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
        // panic if it is already initialized to ensure that only a single ScancodeStream instance can be created.
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            // use the `OnceCell::try_ge` method to get a reference to the initialized scancode queue.
            .try_get()
            .expect("not initialized");
        // use the `ArrayQueue::pop` method to try to get the next element from the queue.
        if let Ok(scancode) = queue.pop() {
            // If it succeeds, we return the scancode wrapped in Poll::Ready(Some(…))
            return Poll::Ready(Some(scancode));
        }

        // If the first call to queue.pop() does not succeed, the queue is potentially empty.
        // Only potentially because the interrupt handler might have filled the queue asynchronously immediately after the check.
        
        // register a wakeup for the passed Waker when it returns Poll::Pending.
        // This way, a wakeup might happen before we return Poll::Pending, but it is guaranteed that we get a wakeup for any scancodes pushed after the check.
        WAKER.register(&cx.waker());
        match queue.pop() {
            Ok(scancode) => {
                // remove the registered waker again using AtomicWaker::take because a waker notification is no longer needed.
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

/// create an asynchronous keyboard task:
pub async fn print_keypresses() {
    let mut scancode = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, 
        HandleControl::Ignore);

    // The code is very similar to the code we had in our keyboard interrupt handler before we modified it in this post. 
    // The only difference is that, instead of reading the scancode from an I/O port, we take it from the ScancodeStream. 
    while let Some(scancode) = scancode.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => print!("{}", character),
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                }
            }
        }
    }
}