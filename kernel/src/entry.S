# Attributes are used to record information about an object file/binary that a
# linker or runtime loader needs to check compatibility.
#
# Reference:
# <https://github.com/riscv-non-isa/riscv-asm-manual/blob/master/riscv-asm.md#-attribute>
.attribute arch, "rv64gc"

.equ NCPU, 2             # Number of CPU
.equ KSTACK_SIZ, 0x10000 # Stack size per hart
.equ HIGH_ADDR, 0xffffffc000000000 # 0xffff_ffc0_0000_0000

.section .text.entry
.globl _entry
_entry:
        # store hartid
        mv tp, a0

        li t0, KSTACK_SIZ
        # li t1, HIGH_ADDR

        la sp, stack0
        add sp, sp, t0

        # make sure high address
        # or sp, sp, t1



        # old
        # la sp, stack0
        # li a0, KSTACK_SIZ  # 64KiB

        # addi a1, tp, 1
        # mul a0, a0, a1
        # add sp, sp, a0

        call main

spin:
        j spin

.section .bss.stack
stack0:
        .space KSTACK_SIZ * NCPU  # -smp 2
