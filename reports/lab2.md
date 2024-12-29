## CH4 实现虚拟地址下的 mmap 和 unmap

首先要实现 `sys_time` 和 `sys_task_info`，这里传入的指针是用户虚拟空间的地址，需要转换为对应的物理地址空间，然后交给内核来写（这里是因为内核采用了直接映射吗，还是内核可以直接写物理地址？应该是直接映射的原因）。借助于提供的 `translated_byte_buffer` 方法，可以实现提取一段虚拟地址对应的物理地址，然后内核往这段物理地址写即可。其实可以根据 `PhysicalAddress` 提供的 `get_mut` 方法来实现，但是基于 `translated_byte_buffer` 可以很好地处理一个结构跨两页的情况，反正实现对物理地址的写即可。

值得注意的是 `PageTable::from_token(token);` 这个函数会新建一个空页表，而不是去访问应用原来的页表。但是利用 `translate` 的方法，可以直接在物理地址上走页表，也可以访问到物理地址，但是这个页表不会有任何的表项。那么如何得到现在任务的页表呢？其实它在 `TaskManager` 中的 `current task` 的 `memory_set` 的 `pagetable`。其实实例只有 `TaskManager`（大写）这么一个东西,还有 `FRAME_ALLOCATOR`，负责全局的物理页帧的分配和回收。

在map和unmap中出现的问题，map比较简单，Memoryset里面提供了insert_frame的方法，根据起始vpn和结束vpn来分配物理页帧和映射，而unmap中，需要借用页表，需要直接访问Memoryset中的页表，但是这个是私用的，需要borrow一个mut的，这里只需要解映射，不需要回收资源，所以直接调用页表的unmap函数即可。然后还有一个check的问题，就是看一个页是否已经被映射，这个直接用translate的方法，看能不能找到pte就行。**问题**：到底是先完成页表的映射还是先完成数据的拷贝，页表映射发生的时间。