use libc::{syscall, SYS_futex, FUTEX_WAIT, FUTEX_WAKE};
use std::{
    panic,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

const UNLOCKED: u32 = 0;
const LOCKED: u32 = 1;

macro_rules! futex_status {
    ($val:expr) => {
        if $val == 0 {
            "UNLOCKED"
        } else {
            "LOCKED"
        }
    };
}

fn main() {
    test_futex();
}

fn futex_wait(futex: &AtomicU32, thread: &str) {
    loop {
        // 如果当前futex没有被其他线程持有
        if futex
            .compare_exchange(UNLOCKED, LOCKED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // 加锁后直接返回，这样就不用执行系统调用，减少一定开销
            println!(
                "线程{thread}上锁成功, futex状态: {}",
                futex_status!(futex.load(Ordering::SeqCst))
            );
            return;
        }

        // 线程进入等待状态
        println!("线程{thread}正在等待futex");
        let ret = unsafe {
            syscall(
                SYS_futex,
                futex as *const AtomicU32 as *mut u32,
                FUTEX_WAIT,
                futex.load(Ordering::SeqCst),
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

fn futex_wake(futex: &AtomicU32, thread: &str) {
    let ret = unsafe {
        syscall(
            SYS_futex,
            futex as *const AtomicU32 as *mut u32,
            FUTEX_WAKE,
            1,
            0,
            0,
        )
    };
    if ret == -1 {
        panic!("futex_wake系统调用执行失败");
    }
    futex.store(UNLOCKED, Ordering::SeqCst);
    println!("线程{thread}释放锁");
}

/// 测试基本的futex使用
fn test_futex() {
    // futex用户空间地址
    let futex = Arc::new(AtomicU32::new(0));
    let futex_clone1 = futex.clone();
    let futex_clone2 = futex.clone();

    // 线程1
    let thread1 = thread::spawn(move || {
        // 尝试获取锁
        futex_wait(&futex_clone1, "1");
        // 执行具体的业务逻辑
        thread::sleep(Duration::from_secs(5));
        // 释放锁
        futex_wake(&futex_clone1, "1");
    });

    // 线程2
    let thread2 = thread::spawn(move || {
        // 尝试获取锁
        futex_wait(&futex_clone2, "2");
        // 执行具体的业务逻辑
        thread::sleep(Duration::from_secs(5));
        // 释放锁
        futex_wake(&futex_clone2, "2");
    });

    thread1.join().unwrap();
    thread2.join().unwrap();
}
