package main

import (
	"fmt"
	"sync"
	"sync/atomic"
	"syscall"
	"time"
	"unsafe"
)

const (
	FUTEX_INIT     uint32 = 0x0000_0000
	FUTEX_WAITS    uint32 = 0x8000_0000
	FUTEX_TID_MASK uint32 = 0x3fff_ffff
	FUTEX_WAIT            = 0
	FUTEX_WAKE            = 1
)

func main() {
	testFutex()
}

func testFutex() {
	var futex atomic.Uint32
	futex.Store(FUTEX_INIT)
	var wg sync.WaitGroup

	wg.Add(2)
	go futexTask(&futex, "1", &wg)
	go futexTask(&futex, "2", &wg)

	wg.Wait()
}

// goroutine
func futexTask(futex *atomic.Uint32, thread string, wg *sync.WaitGroup) {
	defer wg.Done()

	tid := syscall.Gettid()
	// 尝试获取锁
	futexWait(futex, thread, int64(tid))
	// 执行具体的业务逻辑
	time.Sleep(5 * time.Second)
	// 释放锁
	futexWake(futex, thread)
}

func futexWait(futex *atomic.Uint32, thread string, tid int64) {
	for {
		// 如果当前futex没有被其他线程持有
		if (futex.Load() & FUTEX_TID_MASK) == 0 {
			futex.Swap(uint32(tid))
			// 加锁后直接返回，这样就不用执行系统调用，减少一定开销
			fmt.Printf("线程%v上锁成功, futex值: %x\n", thread, futex.Load())
			return
		}

		// 线程进入等待状态
		futex.Or(FUTEX_WAITS)
		fmt.Printf("线程%v正在等待futex, futex值: %x\n", thread, futex.Load())
		ret, _, errno := syscall.Syscall6(
			syscall.SYS_FUTEX,
			uintptr(unsafe.Pointer(futex)),
			FUTEX_WAIT,
			uintptr(futex.Load()),
			0,
			0,
			0)
		if int32(ret) == -1 {
			panic(fmt.Sprintf("futexWait系统调用执行失败: %v", errno))
		}
	}
}

func futexWake(futex *atomic.Uint32, thread string) {
	ret, _, erron := syscall.Syscall(syscall.SYS_FUTEX, uintptr(unsafe.Pointer(futex)), FUTEX_WAKE, 1)
	if int32(ret) == -1 {
		panic(fmt.Sprintf("futexWake系统调用执行失败: %v", erron))
	}
	futex.Store(FUTEX_INIT)
	fmt.Printf("线程%v释放锁\n", thread)
}
