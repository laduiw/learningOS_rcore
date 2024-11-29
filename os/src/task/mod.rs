//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of `PID_ALLOCATOR` allocates pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod id;
mod manager;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
#[allow(rustdoc::private_intra_doc_links)]
mod task;

use crate::fs::{open_file, OpenFlags};
use crate::mm::{MapPermission, VirtPageNum};
use alloc::sync::Arc;
pub use context::TaskContext;
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use id::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
pub use manager::add_task;
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
    Processor,
};
/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        panic!("All applications completed!");
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    // drop file descriptors
    inner.fd_table.clear();
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("ch6b_initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}

/// update syscall_time
pub fn change_syscall_times(syscall_id: usize) {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.task_syscall[syscall_id] += 1;
    //if syscall_id==410 {println!("info time is {}",inner.task_syscall[syscall_id]);}
}

/// check if a vpn is valid
pub fn check(vpn:VirtPageNum) -> bool {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if let Some(pte) = inner.memory_set.translate(vpn) {
        if pte.is_valid() {
            return true;
        } else {
            return false;
        }
    } else {
        return false;
    }
}

/// map a area from start_vpn to end_vpn
pub fn map_all(start_vpn:VirtPageNum,end_vpn:VirtPageNum,port:usize) {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.memory_set.insert_framed_area(start_vpn.into(), end_vpn.into(),  MapPermission::from_bits_truncate(port as u8));
}

/// unmap a area from start_vpn to end_vpn
pub fn unmap_all(start_vpn:VirtPageNum,end_vpn:VirtPageNum) {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    // for 循环不能包括末尾
    for vpn in  start_vpn.0 .. end_vpn.0 {
        inner.memory_set.get_page_table().unmap(VirtPageNum(vpn));
    }
    //inner.tasks[current].memory_set.insert_framed_area(start_vpn.into(), end_vpn.into(),  MapPermission::from_bits_truncate(port as u8));
}

/// spawn a new profess from elf_data
pub fn spawn(elf_data: &[u8]) -> Arc<TaskControlBlock>
{
    let task = current_task().unwrap();
    let mut parent_inner = task.inner_exclusive_access();
    let new_task = Arc::new(TaskControlBlock::new(elf_data));
    //let new_pid = new_task.pid.0;
    parent_inner.children.push(new_task.clone());
    new_task
}