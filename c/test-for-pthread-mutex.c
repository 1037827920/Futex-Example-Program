#include <stdio.h>
#include <pthread.h>
#include <unistd.h>

// 共享计数器
int shread_cnt = 0;
// 互斥锁
pthread_mutex_t cnt_mutex = PTHREAD_MUTEX_INITIALIZER;

// 线程执行的任务
void* thread_task(void* arg) {
    // 线程ID
    long tid = (long)arg;

    // 循环5次，每次增加计数器
    for (int i = 0; i < 5; ++i) {
        // 加锁
        pthread_mutex_lock(&cnt_mutex);

        // 临界区：修改共享资源
        int tmp = shread_cnt;
        // 模拟一些处理时间
        usleep(100);
        shread_cnt = tmp + 1;

        printf("线程 %ld: 计数器值 = %d\n", tid, shread_cnt);

        // 解锁
        pthread_mutex_unlock(&cnt_mutex);

        // 模拟一些处理时间
        usleep(200);
    }

    return NULL;
}

int main() {
    // 定义线程句柄数组
    pthread_t threads[3];

    // 创建3个线程
    for (long i = 0;i < 3; ++i) {
        int ret = pthread_create(&threads[i], NULL, thread_task, (void*)i);

        if (ret != 0) {
            perror("线程创建失败");
            return 1;
        }
    }

    // 等待所有线程完成
    for (int i = 0; i < 3; ++i) {
        pthread_join(threads[i], NULL);
    }

    // 打印最终计数器值
    printf("最终计数器值 = %d\n", shread_cnt);

    // 销毁互斥锁
    pthread_mutex_destroy(&cnt_mutex);

    return 0;
}