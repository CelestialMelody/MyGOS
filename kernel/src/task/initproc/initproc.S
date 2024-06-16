# kernel/task/initproc/initproc.S
#
# All of these below will become a part of the kernel.

.align 3
.section .data
.globl initproc_entry
.globl initproc_tail
initproc_entry:
	.incbin "../crates/libd/target/riscv64gc-unknown-none-elf/release/initproc"
initproc_tail:

.align 3
.section .data
.globl busybox_entry
.globl busybox_tail
busybox_entry:
	.incbin "src/task/initproc/busybox"
busybox_tail:

.align 3
.section .data
.globl test_all_custom_entry
.globl test_all_custom_tail
test_all_custom_entry:
	.incbin "src/task/initproc/test_all_custom.sh"
test_all_custom_tail:

.align 3
.section .data
.globl busybox_testcode_entry
.globl busybox_testcode_tail
busybox_testcode_entry:
	.incbin "src/task/initproc/busybox_testcode.sh"
busybox_testcode_tail:

.align 3
.section .data
.globl busybox_test_cmd_entry
.globl busybox_test_cmd_tail
busybox_test_cmd_entry:
	.incbin "src/task/initproc/busybox_cmd.txt"
busybox_test_cmd_tail: