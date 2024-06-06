# Attributes are used to record information about an object file/binary that a
# linker or runtime loader needs to check compatibility.
#
# Reference:
# <https://github.com/riscv-non-isa/riscv-asm-manual/blob/master/riscv-asm.md#-attribute>
.attribute arch, "rv64gc"

.equ NCPU, 2             # Number of CPU
.equ KSTACK_SIZ, 0x10000  # Stack size per hart

.section .text.entry
.globl _entry
_entry:
        # store hartid
        mv tp, a0

        # Set up the stack pointer
        li t0, KSTACK_SIZ
        la sp, stack0
        add sp, sp, t0

        call main

spin:
        j spin

.section .bss.stack
stack0:
        .space KSTACK_SIZ * NCPU  # -smp 2
