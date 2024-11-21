//! Process management syscalls
extern crate alloc;
use crate::config::PAGE_SIZE;
//use alloc::vec::Vec;
//use alloc::vec;
//use crate::mm::page_table::PageTable;

use crate::mm::VirtAddr;
use crate::task::{check, map_all, unmap_all};
use crate::{
    config::MAX_SYSCALL_NUM, mm::translated_byte_buffer, task::{
        change_program_brk, current_user_token, exit_current_and_run_next, get_running_task_syscall, get_running_task_time, suspend_current_and_run_next, TaskStatus
    }, timer::{get_time_ms, get_time_us}
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/* 
fn vec_of_slices_to_mut_slice(slices: Vec<&mut [u8]>) -> Vec<u8> {
    // 计算总长度
    let total_len: usize = slices.iter().map(|slice| slice.len()).sum();

    // 创建一个新的连续缓冲区
    let mut contiguous_buffer = vec![0u8; total_len];

    // 将每个切片拷贝到新缓冲区
    let mut offset = 0;
    for slice in slices {
        let len = slice.len();
        contiguous_buffer[offset..offset + len].copy_from_slice(slice);
        offset += len;
    }

    contiguous_buffer
}


fn timeval_to_bytes(timeval: &TimeVal) -> Vec<u8> {
    // 将 sec 和 usec 转换为字节数组
    let sec_bytes = timeval.sec.to_le_bytes();
    let usec_bytes = timeval.usec.to_le_bytes();

    // 拼接 sec 和 usec 的字节
    [sec_bytes.as_slice(), usec_bytes.as_slice()].concat()
}*/

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
/// 主要问题是_ts是用户空间传来的地址，而如果直接对*_ts这么写，会根据内核的页表来转换这个地址，导致错误，所以需要利用用户的页表来进行地址转换，得到ppn
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let physical_vec = translated_byte_buffer(
        current_user_token(),
        _ts as *const u8, core::mem::size_of::<TimeVal>()
    );
    let us = get_time_us();
    let ref time_val = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
    };
    let time_write = time_val as *const TimeVal;
    for (page_id, phy) in physical_vec.into_iter().enumerate() {
        let ulen = phy.len();
        unsafe {
            phy.copy_from_slice(core::slice::from_raw_parts(
                // 这里按照byte的方式写入每个物理地址
                time_write.wrapping_byte_add(page_id * ulen) as *const u8,
                ulen)
            );
        }
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    //trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let time_bef = get_running_task_time();
    
    let ref task_now = TaskInfo{
        status: TaskStatus::Running,
        time :  get_time_ms() as usize - time_bef,
        syscall_times: get_running_task_syscall()
    };
    //let t1 = get_time_ms() as usize;
    //println!("time bef is {}, time now is {}", time_bef,t1);

    let physical_vec = translated_byte_buffer(
        current_user_token(),
        _ti as *const u8, core::mem::size_of::<TaskInfo>()
    );

    let task_write = task_now as *const TaskInfo;
    for (page_id, phy) in physical_vec.into_iter().enumerate() {
        let ulen = phy.len();
        unsafe {
            phy.copy_from_slice(core::slice::from_raw_parts(
                task_write.wrapping_byte_add(page_id * ulen) as *const u8,
                ulen)
            );
        }
    }
    
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    //trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if (_port & !0x7!=0) ||  (_port & 0x7 == 0) || (_start & (PAGE_SIZE - 1)!=0){
        return -1;
    }
    //let page_table = PageTable::from_token(current_user_token());
    let mut start_vpn=VirtAddr::from(_start).floor();
    let end_vpn = VirtAddr::from(_start + _len).ceil();
    //println!("task is {}, mmap st_vpn is {}, end_vpn is {}", get_current_task(),start_vpn.0,end_vpn.0);

    while start_vpn.0<end_vpn.0
    {
        //let inner = TASK_MANAGER.inner.exclusive_access();
        if check(start_vpn) {return -1;}
        start_vpn.0 +=1;
    }
    start_vpn=VirtAddr::from(_start).floor();
    map_all(start_vpn,end_vpn,_port<<1 | (1<<4));
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    if _start & (PAGE_SIZE-1)!=0 {return -1;}
    //trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let mut start_vpn=VirtAddr::from(_start).floor();
    let end_vpn = VirtAddr::from(_start + _len).ceil();
    //println!("task is {}, unmap st_vpn is {}, end_vpn is {}", get_current_task(),start_vpn.0,end_vpn.0);
    while start_vpn.0<=end_vpn.0
    {
        //let inner = TASK_MANAGER.inner.exclusive_access();
        if !check(start_vpn) {return -1;}
        start_vpn.0 +=1;
    }
    start_vpn=VirtAddr::from(_start).floor();
    unmap_all(start_vpn,end_vpn);

    0
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
