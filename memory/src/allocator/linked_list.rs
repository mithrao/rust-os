use super::align_up;
use core::mem;

use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

struct ListNode {
    size: usize,
    // The &'static mut type semantically describes an owned object behind a pointer. Basically, it’s a Box without a destructor that frees the object at the end of the scope.
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    // Note that any use of mutable references in const functions (including setting the next field to None) is still unstable. In order to get it to compile, we need to add #![feature(const_mut_refs)] to the beginning of our lib.rs.
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// Creates an empty LinkedListAllocator.
    pub const fn new() -> Self {
        Self { head: ListNode::new(0) }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// Adds the given memory region to the front of the list.
    /// provides the fundamental push operation on the linked list.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // ensure that the freed region is capable of holding ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        // create a new list node and append it at the start of the list
        // 1.  creates a new node on its stack with the size of the freed region
        let mut node = ListNode::new(size);
        // 2. uses the Option::take method to set the next pointer of the node to the current head pointer, 
        //    thereby resetting the head pointer to None.
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        // 3. writes the newly created node to the beginning of the freed memory region through the write method.
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }

    /// Looks for a free region with the given size and alignment and removes
    /// it from the list.
    ///
    /// Returns a tuple of the list node and the start address of the allocation.
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut ListNode, usize)>
    {
        // reference to current list node, update for each iteration
        let mut current = &mut self.head;
        // look for a large enough memory region in linked list
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // region suitable for allocation -> remove node from list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // region is not suitable -> continue with next region
                current = current.next.as_mut().unwrap();
            }
        }
        // When the current.next pointer becomes None, the loop exits. This means we iterated over the whole list but found no region suitable for an allocation.
        // no suitable region found
        None
    }

    /// Try to use the given region for an allocation with given size and
    /// alignment.
    ///
    /// Returns the allocation start address on success.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize)
        -> Result<usize, ()>
    {
        // calculates the start and end address of a potential allocation
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end < region.end_addr() {
            // region too small
            return Err(());
        }

        // This part of the region must store its own ListNode after the allocation, so it must be large enough to do so.
        // The check verifies exactly that: either the allocation fits perfectly (excess_size == 0) or the excess size is large enough to store a ListNode.
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            // rest of region too small to hold a ListNode (required because the
            // allocation splits the region in a used and a free part)
            return Err(());
        }
        
        // region suitable for allocation
        Ok(alloc_start)
    }

    /// Adjust the given layout so that the resulting allocated memory
    /// region is also capable of storing a `ListNode`.
    ///
    /// Returns the adjusted size and alignment as a (size, align) tuple.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            // 1.1 uses the align_to method on the passed Layout to increase the alignment to the alignment of a ListNode if necessary.
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            // 1.2 uses the pad_to_align method to round up the size to a multiple of the alignment to ensure that the start address of the next memory block will have the correct alignment for storing a ListNode too.
            .pad_to_align();
        // 2. uses the max method to enforce a minimum allocation size of mem::size_of::<ListNode>.
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }

}


/// Implementing GlobalAlloc
/// As with the bump allocator, we don’t implement the trait directly for the LinkedListAllocator but only for a wrapped Locked<LinkedListAllocator>.
/// The Locked wrapper adds interior mutability through a spinlock, which allows us to modify the allocator instance even though the alloc and dealloc methods only take &self references.
unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // perform layout adjustments
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        // find a suitable memory region for the allocation and remove it from the list.
        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            // In the success case, the find_region method returns a tuple of the suitable region (no longer in the list) and the start address of the allocation.
            // calculates the end address of the allocation and the excess size again.
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            // If the excess size is not null, it calls add_free_region to add the excess size of the memory region back to the free list
            if excess_size > 0 {
                allocator.add_free_region(alloc_end, excess_size);
            }
            // returns the alloc_start address casted as a *mut u8 pointer.
            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // perform layout adjustments
        let (size, _) = LinkedListAllocator::size_align(layout);

        // retrieves a &mut LinkedListAllocator reference by calling the Mutex::lock function on the Locked wrapper.
        // calls the [add_free_region] function to add the deallocated region to the free list.
        self.lock().add_free_region(ptr as usize, size)
    }
}


