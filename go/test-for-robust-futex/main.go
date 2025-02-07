package main

import (
	"fmt"
	"sync/atomic"
	"syscall"
	"unsafe"
)

const (
	FUTEX_INIT     uint32 = 0x0000_0000
	FUTEX_WAITS    uint32 = 0x8000_0000
	FUTEX_TID_MASK uint32 = 0x3fff_ffff
	FUTEX_WAIT            = 0
	FUTEX_WAKE            = 1
)

type RobustList struct {
	next *RobustList
}

type RobustListHead struct {
	list            RobustList
	futex_offset    int64
	list_op_pending *RobustList
}

func NewRobustListHead(futex_offset int64) *RobustListHead {
	return &RobustListHead{
		list: RobustList{
			next: nil,
		},
		futex_offset:    futex_offset,
		list_op_pending: nil,
	}
}

func (rlh *RobustListHead) PrintRobustList() {
	current := rlh.list.next
	for current != nil {
		futex_ptr := (*atomic.Uint32)(unsafe.Pointer(uintptr(unsafe.Pointer(current)) + uintptr(rlh.futex_offset)))
		fmt.Printf("%p(futex: %p, futex_val: 0x%x) -> ", current, futex_ptr, futex_ptr.Load())
		current = current.next
	}
	fmt.Println("NULL")
}

func (rlh *RobustListHead) GetFutex(index int) *atomic.Uint32 {
	current := rlh.list.next
	for i := 0; i < index; i++ {
		if current == nil {
			return nil
		}
		current = current.next
	}
	if current == nil {
		return nil
	}
	return (*atomic.Uint32)(unsafe.Pointer(uintptr(unsafe.Pointer(current)) + uintptr(rlh.futex_offset)))
}

func main() {
	testSetAndGetRobustList()
	testRobustFutex()
}

// func futexWait(futex *atomic.Uint32, thread string, tid int64) {
// 	for {
// 		fmt.Printf("线程%v尝试上锁, futex值: 0x%x\n", thread, futex.Load())
// 		// 如果当前futex没有被其他线程持有
// 		if (futex.Load() & FUTEX_TID_MASK) == 0 {
// 			futex.Swap(uint32(tid))
// 			// 加锁后直接返回，这样就不用执行系统调用，减少一定开销
// 			fmt.Printf("线程%v上锁成功, futex值: 0x%x\n", thread, futex.Load())
// 			return
// 		}

// 		// 线程进入等待状态
// 		futex.Or(FUTEX_WAITS)
// 		fmt.Printf("线程%v正在等待futex, futex值: 0x%x\n", thread, futex.Load())
// 		ret, _, errno := syscall.Syscall6(
// 			syscall.SYS_FUTEX,
// 			uintptr(unsafe.Pointer(futex)),
// 			FUTEX_WAIT,
// 			uintptr(futex.Load()),
// 			0,
// 			0,
// 			0)
// 		if int32(ret) == -1 {
// 			panic(fmt.Sprintf("futexWait系统调用执行失败: %v", errno))
// 		}
// 	}
// }

// func futexWake(futex *atomic.Uint32, thread string) {
// 	ret, _, erron := syscall.Syscall(syscall.SYS_FUTEX, uintptr(unsafe.Pointer(futex)), FUTEX_WAKE, 1)
// 	if int32(ret) == -1 {
// 		panic(fmt.Sprintf("futexWake系统调用执行失败: %v", erron))
// 	}
// 	futex.Store(FUTEX_INIT)
// 	fmt.Printf("线程%v释放锁\n", thread)
// }

func setRobustList(robustListHeadPtr *RobustListHead) {
	ret, _, errno := syscall.Syscall(
		syscall.SYS_SET_ROBUST_LIST,
		uintptr(unsafe.Pointer(robustListHeadPtr)),
		unsafe.Sizeof(RobustListHead{}),
		0,
	)
	if int32(ret) == -1 {
		panic(fmt.Sprintf("setRobustList系统调用执行失败: %v", errno))
	}
}

func getRobustList(tid int, robustListHeadPtr **RobustListHead, len *uint) {
	ret, _, errno := syscall.Syscall(
		syscall.SYS_GET_ROBUST_LIST,
		uintptr(tid),
		uintptr(unsafe.Pointer(unsafe.Pointer(robustListHeadPtr))),
		uintptr(unsafe.Pointer(len)),
	)
	if int32(ret) == -1 {
		panic(fmt.Sprintf("getRobustList系统调用执行失败: %v", errno))
	}
}

// 测试setRobustList系统调用和gerRobustList系统调用
func testSetAndGetRobustList() {
	// 初始化robust list head
	// 8字节为RobustList的大小
	robust_list_head := NewRobustListHead(8)

	// 初始化robust list
	robust_list := (*RobustList)(unsafe.Pointer(&[16]byte{}))
	futex_ptr := (*atomic.Uint32)(unsafe.Pointer(uintptr(unsafe.Pointer(robust_list)) + uintptr(robust_list_head.futex_offset)))
	futex_ptr.Store(FUTEX_INIT)
	robust_list_head.list.next = robust_list

	// 打印robust list
	robust_list_head.PrintRobustList()

	// 设置robust list
	setRobustList(robust_list_head)

	// 获取robust list
	var robust_list_head_ptr_geted *RobustListHead
	var len_geted uint
	getRobustList(0, &robust_list_head_ptr_geted, &len_geted)
	fmt.Printf("robust_list_head_ptr_geted: %p, robust_list_head%+v, len_geted: %v\n", robust_list_head_ptr_geted, *robust_list_head_ptr_geted, len_geted)
}

// 测试一个线程异常退出时，robust list的表现
func testRobustFutex() {
	// 这个暂时弃坑了，因为goroutine并不是线程，就是创建两个goroutine，实际上可能开了6、7个线程，然后退出goroutine运行完可能还是没退出
	// 跟正常的线程的不太一样，我还没玩明白，还是暂时不钻这个牛角尖了
}
