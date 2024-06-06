# kernel/trap/trampoline.S

	# TrapContext memory layout:
    #
    #     Offset | Explanation
    # -----------------------------------------
	#          0 | 32 genernal purpose registers
	#  32*8(256) | sstatus
	#  33*8(264) | sepc
	#  34*8(272) | kernel_satp
	#  35*8(280) | kernel_sp
	#  36*8(288) | trap_handler
	#  37*8(296) | executor_id
    #  38*8(304) | float registers

.attribute arch, "rv64gc"

.altmacro
.macro STORE_X n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_X n
    ld x\n, \n*8(sp)
.endm
.macro STORE_FP n, m
    fsd f\n, \m*8(sp)
.endm
.macro LOAD_FP n, m
    fld f\n, \m*8(sp)
.endm

.section .text.trampoline
.globl user_trapvec
.align 2
user_trapvec:
	# sscratch 中存放着 trap 上下文的虚拟地址
	# sp 为用户栈的栈顶地址
	# 交换 sp 和 sscratch 的值
	csrrw sp, sscratch, sp

	# 保存除了 x0, sp 的其他通用寄存器
	sd ra, 1*8(sp)
	sd gp, 3*8(sp)

	.set n, 4
	.rept 28
		STORE_X %n
		.set n, n+1
	.endr

	# t0, t1 因为前面已经存到内存了, 现在可以随便使用
	csrr t0, sstatus
	csrr t1, sepc
	sd t0, 32*8(sp)
	sd t1, 33*8(sp)

	# read user stack from sscratch and save it in TrapContext
	# 此时 sscratch 寄存器的值为用户栈栈帧 sp, 读取并保存至 TrapContext
	csrr t2, sscratch
	sd t2, 2*8(sp)

    # 保存浮点寄存器
    .set n, 0
    .set m, 38
    .rept 32
        STORE_FP %n, %m
        .set n, n+1
        .set m, m+1
    .endr

	# 将 kernel_satp 装载到 t0
	ld t0, 34*8(sp)

	# 将 trap_handler 的入口地址装载到 t1
	ld t1, 36*8(sp)

    ld tp, 37*8(sp)

	# 将 sp 设置到内核栈
	ld sp, 35*8(sp)

    ##                    ##
	## 				      ##
	###### USER SPACE ######


	# 切换到内核地址空间
	csrw satp, t0

	# 刷新 TLB 缓存
	sfence.vma

	##### KERNEL SPACE #####
	##                    ##
	##                    ##

	# 跳转到 trap 处理函数
	jr t1

.globl user_trapret
.align 2
user_trapret:
	# 进到当前代码前由 Rust 代码设置:
	# a0 = TRAP_CONTEXT;         // TrapContext 在用户地址空间中的虚拟地址
	# a1 = current_user_token(); // 当前任务的 satp (用户地址空间的token)


	##					  ##
	##					  ##
	##### KERNEL SPACE #####

	# 切换到应用地址空间
	csrw satp, a1

	# 刷新 MMU 的 TLB
	sfence.vma

	###### USER SPACE ######
	##                    ##
	##                    ##

	# 将 TrapContext 的值存入 sscratch, 供 user_trapvec 使用
	csrw sscratch, a0
	# 将 a0 的值转存到 sp, 是为了使用宏
	mv sp, a0

	# 在恢复所有通用寄存器前, 通用寄存器中大部分可以随意使用, 这里选择
	# t0, t1 两个寄存器, 用来将内存中的 sstatus 和 sepc 恢复到相应寄存器
	ld t0, 32*8(sp)
	ld t1, 33*8(sp)
	csrw sstatus, t0
	csrw sepc, t1

    .set n, 0
    .set m, 38
    .rept 32
        LOAD_FP %n, %m
        .set n, n+1
        .set m, m+1
    .endr

    sd tp, 37*8(sp)

	# 开始恢复除了 x0 和 sp 以外的通用寄存器
	# 因为 sp 在宏中是作为偏移的基址使用的, 所以现在不能从内存中恢复它的值
	ld ra, 1*8(sp)
	ld gp, 3*8(sp)
	.set n, 4
	.rept 28
		LOAD_X %n
		.set n, n+1
	.endr

	# 其他通用寄存器(除 x0 外), 已全部恢复, 恢复 sp
	ld sp, 2*8(sp)

	sret

.section .text.signal
.globl user_sigreturn
.align 2
user_sigreturn:
    li	a7, 139 # syscall SIGRETURN
    ecall
