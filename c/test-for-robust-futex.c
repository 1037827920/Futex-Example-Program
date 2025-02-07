#include <stdio.h>
#include <linux/futex.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <stdint.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <pthread.h>

#define FUTEX_INIT 0x00000000
#define FUTEX_WAITERS 0x80000000
#define FUTEX_TID_MASK 0x3fffffff

// 定义一个结构体来封装线程任务参数
typedef struct {
    struct robust_list_head* rlh;
    int thread;
} thread_task_args;

// 获取索引为index的futex地址
atomic_uint* robust_list_get_futex(struct robust_list_head* rlh, size_t index) {
    struct robust_list* current = rlh->list.next;
    size_t i = 0;
    while (current != NULL) {
        if (i == index) {
            return (atomic_uint*)((uint8_t*)current + rlh->futex_offset);
        }
        current = current->next;
        i++;
    }
    return NULL;
}

// 打印robust list
void robust_list_print(struct robust_list_head* rlh) {
    struct robust_list* current = rlh->list.next;
    while (current != NULL) {
        atomic_uint* futex_ptr = (atomic_uint*)((uint8_t*)current + rlh->futex_offset);
        atomic_int futex = *futex_ptr;

        printf("%p(futex: %p, futex_val: %u) -> ", (void*)current, (void*)futex_ptr, futex);

        current = current->next;
    }
    printf("NULL\n");
}

// 向kernel注册robust list
void set_robust_list(struct robust_list_head* rlh) {
    long ret = syscall(SYS_set_robust_list, rlh, sizeof(struct robust_list_head));
    if (ret == -1) {
        perror("set_robust_list系统调用执行失败\n");
    }
}

// 从kernel获取robust list
void get_robust_list(int pid, struct robust_list_head** rlh, size_t* len) {
    long ret = syscall(SYS_get_robust_list, pid, rlh, len);
    if (ret == -1) {
        perror("get_robust_list系统调用执行失败\n");
    }
}

// futex_wait函数
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

// futex_wake函数
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

// 测试set_robust_list系统调用和get_robust_list系统调用
void test_set_and_get_robust_list() {
    // 创建一个robust list head
    struct robust_list_head* rlh = (struct robust_list_head*)malloc(sizeof(struct robust_list_head));
    rlh->futex_offset = 8;

    // 初始化robust list
    struct robust_list* new_robust_list = (struct robust_list*)malloc(sizeof(struct robust_list));
    if (new_robust_list == NULL) {
        perror("new robust list分配内存失败");
    }
    // 通过偏移量将futex写入新robust list
    atomic_uint* futex_ptr = (atomic_uint*)((uint8_t*)new_robust_list + rlh->futex_offset);
    *futex_ptr = 0x00000000;
    rlh->list.next = new_robust_list;
    robust_list_print(rlh);

    // 设置robust list
    printf("robust_list_head_ptr: %p, len: %ld\n", rlh, sizeof(struct robust_list_head));
    set_robust_list(rlh);

    // 获取robust list
    struct robust_list_head* rlh_geted;
    size_t len_geted;
    get_robust_list(0, &rlh_geted, &len_geted);
    printf("robust_list_head_geted_ptr: %p, len_geted: %ld\n", rlh_geted, len_geted);

    free(new_robust_list);
    free(rlh);
}

void* thread1_task(void* arg) {
    thread_task_args* args = (thread_task_args*)arg;
    // robust list head指针
    struct robust_list_head* rlh = args->rlh;
    // 线程号
    int thread = args->thread;
    // TID
    long tid = syscall(SYS_gettid);

    // 注册robust list
    set_robust_list(rlh);

    // 尝试获取锁
    atomic_uint* futex = robust_list_get_futex(rlh, 0);
    futex_wait(futex, thread, tid);
    // 执行具体的业务逻辑
    sleep(5);

    // 线程异常退出
    printf("线程%d异常退出\n", thread);
    pthread_exit(NULL);
}

void* thread2_task(void* arg) {
    thread_task_args* args = (thread_task_args*)arg;
    // robust list head指针
    struct robust_list_head* rlh = args->rlh;
    // 线程号
    int thread = args->thread;
    // TID
    long tid = syscall(SYS_gettid);

    // 注册robust list
    set_robust_list(rlh);

    // 尝试获取锁
    atomic_uint* futex = robust_list_get_futex(rlh, 0);
    futex_wait(futex, thread, tid);
    // 执行具体的业务逻辑
    sleep(5);
    // 释放锁
    futex_wake(futex, thread);
    return NULL;
}

// 测试一个线程异常退出时，robust list的表现
void test_robust_futex() {
    // 线程句柄
    pthread_t t1, t2;

    // 创建robust list head
    struct robust_list_head* rlh = (struct robust_list_head*)malloc(sizeof(struct robust_list_head));

    // 初始化robust list
    struct robust_list* new_robust_list = (struct robust_list*)malloc(sizeof(struct robust_list));
    if (new_robust_list == NULL) {
        perror("new robust list分配内存失败");
    }
    // 通过偏移量将futex写入新robust list
    atomic_uint* futex_ptr = (atomic_uint*)((uint8_t*)new_robust_list + rlh->futex_offset);
    *futex_ptr = 0x00000000;
    rlh->list.next = new_robust_list;

    // 构建线程参数
    thread_task_args args1 = { rlh, 1 };
    thread_task_args args2 = { rlh, 2 };

    // 线程1
    pthread_create(&t1, NULL, thread1_task, (void*)&args1);

    // 等线程1先获取futex锁
    sleep(3);

    // 线程2
    pthread_create(&t2, NULL, thread2_task, (void*)&args2);

    // 等待线程结束
    pthread_join(t1, NULL);
    pthread_join(t2, NULL);
    
    free(new_robust_list);
    free(rlh);
}

int main() {
    test_set_and_get_robust_list();
    test_robust_futex();
}