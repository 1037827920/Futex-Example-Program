#include <stdio.h>
#include <pthread.h>
#include <stdatomic.h>
#include <unistd.h>
#include <sys/syscall.h>
#include <linux/futex.h>

#define FUTEX_INIT 0x00000000
#define FUTEX_WAITERS 0x80000000
#define FUTEX_TID_MASK  0x3fffffff

// 定义一个结构体来封装参数
typedef struct {
    atomic_uint* futex;
    int thread;
} thread_args;

void* futex_wait(atomic_uint* futex, int thread, long tid) {
    while (1) {
        // 如果当前futex没有其他线程持有
        if ((*futex & FUTEX_TID_MASK) == 0) {
            atomic_exchange(futex, (unsigned int)tid);
            // 加锁后直接返回
            printf("线程%d上锁成功. futex值: 0x%x\n", thread, *futex);
            return NULL;
        }

        // 线程进入等待状态
        atomic_fetch_or(futex, FUTEX_WAITERS);
        printf("线程%d正在等待futex, futex值: 0x%x\n", thread, *futex);
        long ret = syscall(SYS_futex, (unsigned*)futex, FUTEX_WAIT, *futex, 0, 0, 0);
        if (ret == -1) {
            perror("futex_wait系统调用执行失败\n");
            return NULL;
        }
    }
}

void* futex_wake(atomic_uint* futex, int thread) {
    long ret = syscall(SYS_futex, (unsigned*)futex, FUTEX_WAKE, 1, 0, 0, 0);
    if (ret == -1) {
        perror("futex_wake系统调用执行失败\n");
        return NULL;
    }
    atomic_store(futex, FUTEX_INIT);
    printf("线程%d释放锁\n", thread);
    return NULL;
}

void* thread_task(void* arg) {
    thread_args* args = (thread_args*)arg;
    // futex用户空间地址
    atomic_uint* futex = args->futex;
    // 线程号
    int thread = args->thread;
    // TID
    long tid = syscall(SYS_gettid);

    // 尝试获取锁
    futex_wait(futex, thread, tid);
    // 执行具体的业务逻辑
    sleep(5);
    // 释放锁
    futex_wake(futex, thread);

    return NULL;
}

int main() {
    // 线程句柄
    pthread_t t1, t2;

    // futex用户空间地址
    atomic_uint futex = 0;

    thread_args args1 = { &futex, 1 };
    thread_args args2 = { &futex, 2 };

    // 创建两个线程同时递增cnt
    pthread_create(&t1, NULL, thread_task, (void*)&args1);
    pthread_create(&t2, NULL, thread_task, (void*)&args2);

    // 等待线程结束
    pthread_join(t1, NULL);
    pthread_join(t2, NULL);

    return 0;
}