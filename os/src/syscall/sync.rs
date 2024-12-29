use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    /*trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );*/
    //let current_thread = current_task().unwrap();
    //let mut current_thread_inner = current_thread.inner_exclusive_access();
    //let current_tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    //println!("now mute id is {}, running tid is {}",mutex_id,current_tid);
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let pd_need = process_inner.need_deadlock;

    // add to the need list
    let current_thread = current_task().unwrap();
    let mut current_thread_inner = current_thread.inner_exclusive_access();
    current_thread_inner.apply_mutex_id.push(mutex_id);
    drop(current_thread_inner);
    drop(current_thread);

    let tasks = &process_inner.tasks;
    let n = tasks.len();
    let m = process_inner.mutex_list.len();
    let mut work:Vec<i32>= vec![1; m];
    let mut need: Vec<Vec<i32>> = vec![vec![0; m]; n];
    let mut allocation: Vec<Vec<i32>> = vec![vec![0; m]; n];
    let mut i = 0;

    for task_p in tasks 
    {
        if let Some(arc_task) = task_p
        { 
            let task = Arc::clone(arc_task);
            let task_inner = task.inner_exclusive_access();
            //let tid = task_inner.res.as_ref().unwrap().tid;
            //println!("tid is {}",tid);
            //if  tid == current_tid {
            //    need[i][mutex_id] = 1;
            //}
            //let mut j = 0;
            let task_apply_mutex_id = &task_inner.apply_mutex_id;
            for id in task_apply_mutex_id {
                need[i][*id] = need[i][*id] + 1;
            }

            let task_allocated_mutex_id = &task_inner.allocated_mutex_id;
            for id in task_allocated_mutex_id {
                //println!("mutex id is {}", id);
                work[*id] = 0;
                allocation[i][*id] = 1;
                //j = j + 1;
            }
        }
        i = i + 1;
    }
    drop(process_inner);
    drop(process);
    if pd_need==0 || check(n,m,&mut work,&need,&allocation)==true {
        let current_thread = current_task().unwrap();
        let mut current_thread_inner = current_thread.inner_exclusive_access();

        // delete the mutex id in need list
        let current_apply_mutex_id = &mut current_thread_inner.apply_mutex_id;
        if let Some(pos) = current_apply_mutex_id.iter().position(|&x| x == mutex_id) {
            //println!("remove sem id is {}",sem_id);
            current_apply_mutex_id.remove(pos); 
        }

        // add the mutex into the allocation list
        let current_allocated_mutex_id = &mut current_thread_inner.allocated_mutex_id;
        current_allocated_mutex_id.push(mutex_id);
        drop(current_thread_inner);
        drop(current_thread);

        mutex.lock();
        //let current_thread = current_task().unwrap();
        //let mut current_thread_inner = current_thread.inner_exclusive_access();
        //let current_mutex_id = &mut current_thread_inner.mutex_id;
        //current_mutex_id.push(mutex_id);
        0
    } else {      
        // delete the mutex id in need list
        let current_thread = current_task().unwrap();
        let mut current_thread_inner = current_thread.inner_exclusive_access();
        let current_apply_mutex_id = &mut current_thread_inner.apply_mutex_id;
        if let Some(pos) = current_apply_mutex_id.iter().position(|&x| x == mutex_id) {
            //println!("remove sem id is {}",sem_id);
            current_apply_mutex_id.remove(pos); 
        }
        drop(current_thread_inner);
        drop(current_thread);
        -0xdead
    }
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    // delete mutex in allocation list
    let current_thread = current_task().unwrap();
    let mut current_thread_inner = current_thread.inner_exclusive_access();
    let current_allocated_mutex_id = &mut current_thread_inner.allocated_mutex_id;
    if let Some(pos) = current_allocated_mutex_id.iter().position(|&x| x == mutex_id) {
        //println!("remove sem id is {}",sem_id);
        current_allocated_mutex_id.remove(pos); 
    }
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.semaphore_available[id] = res_count;
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_available.push(res_count);
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    //println!("now remove {}",sem_id);
    let current_thread = current_task().unwrap();
    let mut current_thread_inner = current_thread.inner_exclusive_access();
    let current_allocated_sem_id = &mut current_thread_inner.allocated_sem_id;
    if let Some(pos) = current_allocated_sem_id.iter().position(|&x| x == sem_id) {
        //println!("remove sem id is {}",sem_id);
        current_allocated_sem_id.remove(pos); 
    }
    drop(current_thread_inner);
    drop(current_thread);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
   
    //let current_tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    //println!("");
    //println!("now sem id is {}, tid is {}",sem_id,current_tid); 
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let pd_need = process_inner.need_deadlock;

    // add to the need list
    let current_thread = current_task().unwrap();
    let mut current_thread_inner = current_thread.inner_exclusive_access();
    current_thread_inner.apply_sem_id.push(sem_id);
    drop(current_thread_inner);
    drop(current_thread);

    let tasks = &process_inner.tasks;
    let n = tasks.len();
    let m = process_inner.semaphore_list.len();
    let mut work:Vec<i32>= process_inner.semaphore_available.iter().map(|&x| x as i32).collect();
    let mut need: Vec<Vec<i32>> = vec![vec![0; m]; n];
    let mut allocation: Vec<Vec<i32>> = vec![vec![0; m]; n];
    let mut i = 0;
    for task_p in tasks 
    {
        if let Some(arc_task) = task_p
        { 
            let task = Arc::clone(arc_task);
            let task_inner = task.inner_exclusive_access();
 
            //let mut j = 0;
            let task_apply_sem_id = &task_inner.apply_sem_id;
            for id in task_apply_sem_id {
                need[i][*id] = need[i][*id] + 1;
            }

            let task_allocated_sem_id = &task_inner.allocated_sem_id;
            for id in task_allocated_sem_id {
                //println!("mutex id is {}", id);
                work[*id] = work[*id] - 1;
                allocation[i][*id] = allocation[i][*id] + 1;
                //j = j + 1;
            }
        }
        i = i + 1;
    }
    drop(process_inner);
    drop(process);
    /*println!("print all");
    println!("work: {:?}", work);
    println!("need: {:?}", need);
    println!("allocation: {:?}", allocation);*/

    if pd_need==0 || check(n,m,&mut work,&need,&allocation)==true {
        //println!("OK sem can be allocated");
        sem.down();
        let current_thread = current_task().unwrap();
        let mut current_thread_inner = current_thread.inner_exclusive_access();

        // delete the sem id in need list
        let current_apply_sem_id = &mut current_thread_inner.apply_sem_id;
        if let Some(pos) = current_apply_sem_id.iter().position(|&x| x == sem_id) {
            //println!("remove sem id is {}",sem_id);
            current_apply_sem_id.remove(pos); 
        }

        // add the sem into the allocation list
        let current_allocated_sem_id = &mut current_thread_inner.allocated_sem_id;
        current_allocated_sem_id.push(sem_id);
        drop(current_thread_inner);
        drop(current_thread);
        0
    } else {
        // delete the sem id in need list
        let current_thread = current_task().unwrap();
        let mut current_thread_inner = current_thread.inner_exclusive_access();
        let current_apply_sem_id = &mut current_thread_inner.apply_sem_id;
        if let Some(pos) = current_apply_sem_id.iter().position(|&x| x == sem_id) {
            //println!("remove sem id is {}",sem_id);
            current_apply_sem_id.remove(pos); 
        }
        drop(current_thread_inner);
        drop(current_thread);
        -0xdead
    }

}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    //trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    //println!("enable is {}",enabled);
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.need_deadlock = enabled;
    drop(process_inner);
    drop(process);
    0
}

pub fn check(
    n: usize, // 进程数
    m: usize, // 资源种类数
    work: &mut Vec<i32>,
    need: &Vec<Vec<i32>>,
    allocation: &Vec<Vec<i32>>,
) -> bool {
    let mut finish = vec![false; n];
    let mut safe_sequence = Vec::new(); 
    loop {
        let mut allocated = false; 
        for i in 0..n {
            if finish[i] {
                continue; 
            }

            if (0..m).all(|j| need[i][j] <= work[j]) {
                for j in 0..m {
                    work[j] += allocation[i][j]; 
                }
                finish[i] = true;
                safe_sequence.push(i);
                allocated = true;
                break; 
            }
        }

        if !allocated {
            break; 
        }
    }

    if finish.iter().all(|&f| f) {
        true
    } else {
        false
    }
}