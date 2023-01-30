use alloc::alloc::Layout;
use core::ptr;
use super::Locked;
use alloc::alloc::GlobalAlloc;
use core::{mem, ptr::NonNull};

struct ListNode {
    // we don’t have a size field. It isn’t needed because every block in a list has the same size with the fixed-size block allocator design.
    next: Option<&'static mut ListNode>,
}

/// The block sizes to use.
///
/// The sizes must each be power of 2 because they are also used as
/// the block alignment (alignments must be always powers of 2).
/// 
/// We don’t define any block sizes smaller than 8 because each block must be capable of storing a 64-bit pointer to the next block when freed. 
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// Calculating the list index
/// Choose an appropriate block size for the given layout.
///
/// Returns an index into the `BLOCK_SIZES` array.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

pub struct FixedSizeBlockAllocator {
    // The list_heads field is an array of head pointers, one for each block size. This is implemented by using the len() of the BLOCK_SIZES slice as the array length. 
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    // As a fallback allocator for allocations larger than the largest block size, we use the allocator provided by the linked_list_allocator.
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    /// Creates an empty FixedSizeBlockAllocator.
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            // initializes the list_heads array with empty nodes
            // The EMPTY constant is needed to tell the Rust compiler that we want to initialize the array with a constant value.
            // Initializing the array directly as [None; BLOCK_SIZES.len()] does not work, because then the compiler requires Option<&'static mut ListNode> to implement the Copy trait, which it does not. 
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        // only calls the init function of the fallback_allocator without doing any additional initialization of the list_heads array.
        // Instead, we will initialize the lists lazily on alloc and dealloc calls.
        self.fallback_allocator.init(heap_start, heap_size)
    }

    /// Allocates using the fallback allocator.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            // [allocate_first_fit] returns a Result<NonNull<u8>, ()>
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}


unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    // note: The alloc method is the only place where new blocks are created in our implementation. 
    //       This means that we initially start with empty block lists and only fill these lists lazily when allocations of their block size are performed.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 1. use the Locked::lock method to get a mutable reference to the wrapped allocator instance.
        let mut allocator = self.lock();
        // 2. call the list_index function we just defined to calculate the appropriate block size for the given layout and get the corresponding index into the list_heads array.
        match list_index(&layout) {
            Some(index) => {
                // 3.1 If the list index is Some, we try to remove the first node in the corresponding list started by list_heads[index] using the Option::take method.
                match allocator.list_heads[index].take() {
                    // 4.1 If the list is not empty, we enter the Some(node) branch of the match statement, where we point the head pointer of the list to the successor of the popped node (by using take again)
                    Some(node) => {
                        allocator.list_heads[index] = node.next.take();
                        // 5. return the popped node pointer as a *mut u8
                        node as *mut ListNode as *mut u8
                    }
                    // 4.2 If the list head is None, it indicates that the list of blocks is empty.
                    //     This means that we need to construct a new block
                    None => {
                        // no block exists in list => allocate new block
                        // 5. first get the current block size from the BLOCK_SIZES slice and use it as both the size and the alignment for the new block.
                        let block_size = BLOCK_SIZES[index];
                        // only work if all block sizes are a power of 2
                        let block_align = block_size;
                        // 6. create a new Layout from it and call the fallback_alloc method to perform the allocation.
                        let layout = Layout::from_size_align(block_size, block_align)
                            .unwrap();
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            // 3.2 If this index is None, no block size fits for the allocation, 
            // therefore we use the fallback_allocator using the fallback_alloc function.
            None => allocator.fallback_alloc(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            // If list_index returns a block index, we need to add the freed memory block to the list. 
            Some(index) => {
                // first create a new ListNode that points to the current list head (by using Option::take again).
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                // verify that block has size and alignment required for storing node
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                // perform the write by converting the given *mut u8 pointer to a *mut ListNode pointer and then calling the unsafe write method on it.
                let new_node_ptr = ptr as *mut ListNode;
                new_node_ptr.write(new_node);
                // set the head pointer of the list, which is currently None since we called take on it, to our newly written ListNode. 
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
            // If the index is None, no fitting block size exists in BLOCK_SIZES, 
            // which indicates that the allocation was created by the fallback allocator. 
            None => {
                let ptr = NonNull::new(ptr).unwrap();
                // use fallback_allocator.deallocate to free the memory again.
                // The method expects a NonNull instead of a *mut u8, so we need to convert the pointer first.
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}

