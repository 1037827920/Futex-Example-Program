use libc::{
    syscall, SYS_futex, SYS_get_robust_list, SYS_gettid, SYS_set_robust_list, FUTEX_WAIT,
    FUTEX_WAKE,
};
use std::{
    io, mem, panic, ptr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

const FUTEX_INIT: u32 = 0x0000_0000;
const FUTEX_WAITERS: u32 = 0x8000_0000;
const FUTEX_TID_MASK: u32 = 0x3fff_ffff;

#[repr(C)]
#[derive(Debug, Clone)]
struct RobustList {
    next: *mut RobustList,
    // futex: u32, (通过偏移量访问)
}

#[repr(C)]
#[derive(Debug, Clone)]
struct RobustListHead {
    list: RobustList,
    /// 这个偏移量的作用是为了能够通过（list项的地址+偏移量）得到futex用户空间地址
    /// futex_offset的单位为字节，即1表示一个字节的偏移量
    /// 在用户空间添加这个字段还能进行灵活编码偏移量而不是在内核硬编码
    futex_offset: i64,
    list_op_pending: *mut RobustList,
}

impl RobustListHead {
    fn new(futex_offset: i64) -> RobustListHead {
        RobustListHead {
            list: RobustList {
                next: ptr::null_mut(),
            },
            futex_offset,
            list_op_pending: ptr::null_mut(),
        }
    }

    fn push(&mut self, futex: AtomicU32) {
        let new_node = Box::into_raw(Box::new(RobustList {
            next: self.list.next,
        }));
        unsafe {
            ((new_node as *mut u8).offset(self.futex_offset as isize) as *mut AtomicU32)
                .write(futex);
        }
        self.list.next = new_node;
    }

    fn print_robust_list(&self) {
        let mut current = self.list.next;
        while !current.is_null() {
            unsafe {
                print!(
                    "{:?}(futex: {:?}, futex_val: {:?}) -> ",
                    current,
                    (current as *const u8).add(self.futex_offset as usize),
                    ((current as *const u8).add(self.futex_offset as usize) as *const AtomicU32)
                        .read(),
                );
                current = (*current).next;
            }
        }
        println!("NULL");
    }

    fn get_futex(&self, index: usize) -> *const AtomicU32 {
        let mut current = self.list.next;
        for _ in 0..index {
            if current.is_null() {
                return ptr::null_mut();
            }
            unsafe {
                current = (*current).next;
            }
        }
        if current.is_null() {
            return ptr::null_mut();
        }
        unsafe { (current as *const u8).add(self.futex_offset as usize) as *mut AtomicU32 }
    }
}

unsafe impl Send for RobustListHead {}
unsafe impl Sync for RobustListHead {}

fn main() {
    test_set_and_get_robust_list();
    test_robust_futex();
}

fn futex_wait(futex: *const AtomicU32, thread: &str, tid: i64) {
    let futex_ref = unsafe { &*futex };
    loop {
        println!(
            "线程{thread}尝试上锁, futex值: {:#x}",
            (*futex_ref).load(Ordering::SeqCst)
        );
        // 如果当前futex没有被其他线程持有
        if (futex_ref.load(Ordering::SeqCst) & FUTEX_TID_MASK) == 0 {
            futex_ref.swap(tid as u32, Ordering::SeqCst);
            // 加锁后直接返回，这样就不用执行系统调用，减少一定开销
            println!(
                "线程{thread}上锁成功, futex值: {:#x}",
                (*futex_ref).load(Ordering::SeqCst)
            );
            return;
        }

        // 标识futex的FUTEX_WAITERS位
        futex_ref.fetch_or(FUTEX_WAITERS, Ordering::SeqCst);
        println!(
            "线程{thread}正在等待futex, futex值: {:#x}",
            (*futex_ref).load(Ordering::SeqCst)
        );
        let ret = unsafe {
            syscall(
                SYS_futex,
                futex_ref as *const AtomicU32 as *mut u32,
                FUTEX_WAIT,
                futex_ref.load(Ordering::SeqCst),
                0,
                0,
                0,
            )
        };
        if ret == -1 {
            panic!("futex_wait系统调用执行失败");
        }
    }
}

fn futex_wake(futex: *const AtomicU32, thread: &str) {
    let futex_ref = unsafe { &*futex };
    let ret = unsafe {
        syscall(
            SYS_futex,
            futex_ref as *const AtomicU32 as *mut u32,
            FUTEX_WAKE,
            1,
            0,
            0,
            0,
        )
    };
    if ret == -1 {
        panic!("futex_wake系统调用执行失败");
    }
    futex_ref.store(FUTEX_INIT, Ordering::SeqCst);
    println!("线程{thread}释放锁");
}

/// 向kernel注册一个robust list
fn set_robust_list(robust_list_head_ptr: *const RobustListHead) {
    let ret = unsafe {
        syscall(
            SYS_set_robust_list,
            robust_list_head_ptr,
            mem::size_of::<RobustListHead>(),
        )
    };
    if ret == -1 {
        panic!(
            "set_robust_list系统调用执行失败, Err: {:?}",
            io::Error::last_os_error()
        );
    }
}

/// 获取kernel注册的robust list
/// 当pid=0时，表示获取当前进程的robust list
fn get_robust_list(pid: i32, robust_list_head_ptr: &mut *mut RobustListHead, len: &mut usize) {
    let ret = unsafe { syscall(SYS_get_robust_list, pid, robust_list_head_ptr, len) };
    if ret == -1 {
        panic!(
            "get_robust_list系统调用执行失败, Err: {:?}",
            io::Error::last_os_error()
        );
    }
}

/// 测试set_robust_list系统调用和get_robust_list系统调用
fn test_set_and_get_robust_list() {
    // 初始化robust list head
    // 8字节为RobustList的大小
    let mut robust_list_head = RobustListHead::new(8);

    // 初始化robust list
    robust_list_head.push(AtomicU32::new(FUTEX_INIT));
    robust_list_head.print_robust_list();

    // 设置robust list
    let robust_list_head_ptr = &robust_list_head as *const RobustListHead;
    println!(
        "robust_list_head_ptr: {:?}, len: {:?}",
        robust_list_head_ptr,
        mem::size_of::<RobustListHead>()
    );
    set_robust_list(robust_list_head_ptr);

    // 获取robust list
    let mut robust_list_head_ptr_geted: *mut RobustListHead = ptr::null_mut();
    let mut len_geted: usize = 0;
    get_robust_list(0, &mut robust_list_head_ptr_geted, &mut len_geted);
    println!(
        "robust_list_head_ptr_geted: {:?}, robust_list_head: {:?} len_geted: {:?}",
        robust_list_head_ptr_geted,
        unsafe { &*robust_list_head_ptr_geted },
        len_geted
    );
}

/// 测试一个线程异常退出时，robust list的表现
fn test_robust_futex() {
    // 创建robust list head
    // 8字节为RobustList的大小
    let robust_list_head = Arc::new(Mutex::new(RobustListHead::new(8)));

    // 初始化robust list
    let mut robust_list_head_guard = robust_list_head.lock().unwrap();
    (*robust_list_head_guard).push(AtomicU32::new(FUTEX_INIT));
    drop(robust_list_head_guard);

    // 线程1
    let robust_list_head_clone1 = robust_list_head.clone();
    let thread1 = thread::spawn(move || {
        let tid = unsafe { syscall(SYS_gettid) };
        println!("线程1的线程号: {tid}");

        // 向kernel注册robust list
        let robust_list_head_guard = robust_list_head_clone1.lock().unwrap();
        let robust_list_head_ptr = &(*robust_list_head_guard) as *const RobustListHead;
        set_robust_list(robust_list_head_ptr);

        // 尝试获取锁
        let futex = robust_list_head_guard.get_futex(0);
        futex_wait(futex, "1", tid);
        // 执行具体的业务逻辑
        thread::sleep(Duration::from_secs(5));

        // 模拟线程异常退出，是否会正常把未释放的锁释放掉
        println!("thread1异常退出");
        return;
    });

    thread::sleep(Duration::from_secs(3));

    // 线程2
    let robust_list_head_clone2 = robust_list_head.clone();
    let thread2 = thread::spawn(move || {
        let tid = unsafe { syscall(SYS_gettid) };
        println!("线程2的线程号: {tid}");

        // 向kernel注册robust list
        let robust_list_head_guard = robust_list_head_clone2.lock().unwrap();
        let robust_list_head_ptr = &(*robust_list_head_guard) as *const RobustListHead;
        set_robust_list(robust_list_head_ptr);

        // 尝试获取锁
        let futex = robust_list_head_guard.get_futex(0);
        futex_wait(futex, "2", tid);
        // 执行具体的业务逻辑
        thread::sleep(Duration::from_secs(3));
        // 释放锁
        futex_wake(futex, "2");
    });

    thread1.join().unwrap();
    thread2.join().unwrap();
}
