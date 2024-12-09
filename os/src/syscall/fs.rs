//! File and filesystem-related syscalls
use crate::fs::{find_inode, get_file_nlink, link_file, open_file, unlink_file, OpenFlags, Stat, StatMode};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use crate::task::{current_task, current_user_token};

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(fd: usize, _st: *mut Stat) -> isize {
    //let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    let file_inode: usize;
    let file_type:StatMode;
    let file_inf:(usize,usize);

    if let Some(file) = &inner.fd_table[fd] {
        let file=file.clone();
        drop(inner);
        file_inode = file.get_file_inode();
        file_type = file.get_file_type();
        file_inf = file.get_file_inf();
    }
    else {return -1;}

    let nlink = get_file_nlink(file_inf.0,file_inf.1);
    let stat = &Stat 
    {
        dev: 0,
        ino: file_inode as u64,
        mode: file_type,
        nlink: nlink,
        pad: [0;7],
    };

    let physical_vec = translated_byte_buffer(
        current_user_token(),
        _st as *const u8, core::mem::size_of::<Stat>()
    );
    let stat_write = stat as *const Stat;
    for (page_id, phy) in physical_vec.into_iter().enumerate() {
        let ulen = phy.len();
        unsafe {
            phy.copy_from_slice(core::slice::from_raw_parts(
                stat_write.wrapping_byte_add(page_id * ulen) as *const u8,
                ulen)
            );
        }
    }
    

    //let ino = inner.fd_table[fd];
    0
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(old_name: *const u8, new_name: *const u8) -> isize {
    let task = current_task().unwrap();
    let token = current_user_token();
    let old_name = translated_str(token, old_name);
    let new_name = translated_str(token, new_name);
    if old_name == new_name {
        return -1;
    }
    if let Some(inode) = link_file(&old_name,&new_name) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        0
    } else {
        -1
    }
    

}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(name: *const u8) -> isize {
    let token = current_user_token();
    let name = translated_str(token, name);

    /*if let Some(file) = &inner.fd_table[fd] {
        let file=file.clone();
        drop(inner);
        file_inf = file.get_file_inf();
    }
    else {return -1;}*/
    if let Some(inode) = find_inode(&name) {
        let inf = inode.get_inode_inf();
        let nlink = get_file_nlink(inf.0,inf.1);
        if nlink==1
        {
            inode.clear();
        }
        let q = unlink_file(&name);
        //println!("emp is {}",q.1);
        q.0
    } else {-1}
    }