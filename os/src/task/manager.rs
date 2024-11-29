//!Implementation of [`TaskManager`]
use super::{TaskControlBlock, TaskStatus};
use crate::config::BIGSTRIDE;
//use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        if self.ready_queue.is_empty() {return None}
        let mut prio = 0xFFFFFFFFFFFFFFFF;
        let mut target_index:usize = 0;
        for (index,task) in self.ready_queue.iter().enumerate() {
            let inner = task.inner_exclusive_access();
            if inner.get_status() == TaskStatus::Ready {
                if prio>inner.stride {prio=inner.stride; target_index = index;}
                drop(inner);
            }
        }

        if let Some(task) = self.ready_queue.get(target_index) 
        {
            let mut inner = task.inner_exclusive_access();
            inner.stride += BIGSTRIDE / inner.priority;
        }
        self.ready_queue.remove(target_index)
    }
        //self.ready_queue.pop_front()
}

        

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
