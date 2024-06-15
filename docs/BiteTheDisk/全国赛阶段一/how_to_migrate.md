# 如何移植测试

### 移植步骤

1. 让内核直接运行测例，并且在 trap_hander 中打印出 syscall id
2. 在 [Linux kernel system calls for all architectures](https://marcin.juszkiewicz.com.pl/download/tables/syscalls.html) 中查找 RV64 对应的系统调用名
3. 找到对应的 [man-page](https://man7.org/linux/man-pages/index.html)
4. 根据 man-page 的描述完善内核，完成系统调用



### 如何调试

1. 在对应位置打印 print 信息，具有参考价值的信息包括：pid, sepc, va, syscall_id, 各种函数参数，特别是*syscall的参数*，syscall参数要结合 *[man-page](https://man7.org/linux/man-pages/index.html)* 分析。man-page / syscall 参数中某些 flags/常量需要到源码中查找
2. 与地址相关的报错可排查的点：
   1. 检查 va 的值并结合 memory 布局思考
   2. pte 权限与 map 权限
   3. 是否提前释放了 map_area，一般与进程退出相关
   4. memory 布局是否合理，如 (特别是线程) trap_cx 页，sinal_trampoline 页，kernel_stack 页等等是否有冲突
3. 分析源代码。若错误与地址相关，可在 trap_handler 打印 sepc，结合报错的地址 + 分析移植程序的源代码 + （risv64-unknown-elf-)objdump 出移植程序的可执行文件的汇编文件。在汇编文件中找到 sepc 的值对应汇编语言，结合源代码分析如何程序执行