NPROC = 16
MUSL_PREFIX = riscv64-linux
MUSL_GCC = $(MUSL_PREFIX)-gcc
MUSL_STRIP = $(MUSL_PREFIX)-strip

build_all: busybox lua lmbench libctest iozone libc-bench unix-bench true test_all

busybox: .PHONY
	cp busybox-config busybox/.config
	make -C busybox CC="$(MUSL_GCC) -static" STRIP=$(MUSL_STRIP) -j$(NPROC)
	$(MUSL_STRIP) busybox/busybox
	cp busybox/busybox sdcard/
	cp scripts/busybox/* sdcard/

lua: .PHONY
	make -C lua CC="$(MUSL_GCC) -static" -j $(NPROC)
	$(MUSL_STRIP) lua/src/lua
	cp lua/src/lua sdcard/
	cp scripts/lua/* sdcard/

lmbench: .PHONY
	make -C lmbench build CC="riscv64-linux-gnu-gcc -static" OS=riscv64 -j $(NPROC)
	riscv64-linux-gnu-strip lmbench/bin/riscv64/lmbench_all
	# riscv64-linux-gnu-strip lmbench/bin/riscv64/hello
	cp lmbench/bin/riscv64/lmbench_all sdcard/
	cp lmbench/bin/riscv64/hello sdcard/
	cp scripts/lmbench/* sdcard/

libctest: .PHONY
	make -C libc-test disk -j $(NPROC)
	cp libc-test/disk/* sdcard/
	mv sdcard/run-all.sh sdcard/libctest_testcode.sh

iozone: .PHONY
	make -C iozone linux CC="$(MUSL_GCC) -static" -j $(NPROC)
	$(MUSL_STRIP) iozone/iozone
	cp iozone/iozone sdcard/
	cp scripts/iozone/* sdcard/

libc-bench: .PHONY
	make -C libc-bench CC="$(MUSL_GCC) -static" -j $(NPROC)
	$(MUSL_STRIP) libc-bench/libc-bench
	cp libc-bench/libc-bench sdcard/libc-bench

unix-bench: .PHONY
	make -C UnixBench -j $(NPROC) all
	cp UnixBench/pgms/* sdcard
	cp scripts/unixbench/*.sh sdcard

test_all: .PHONY
	cp scripts/test_all.sh sdcard/test_all.sh

true: .PHONY
	make CC=$(MUSL_GCC) -C true
	$(MUSL_STRIP) true/true
	mkdir -p sdcard/bin
	cp true/true sdcard/bin/

sdcard: build_all .PHONY
	dd if=/dev/zero of=sdcard.img count=62768 bs=1K
	mkfs.vfat -F 32 sdcard.img
	mkdir -p mnt
	mount -t vfat -o user,umask=000,utf8=1 --source sdcard.img --target mnt
	cp -r sdcard/* mnt
	umount mnt

qemu: .PHONY
	cd oscomp-debian && ./run.sh

clean: .PHONY
	make -C busybox clean
	make -C lua clean
	make -C lmbench clean
	make -C libc-test clean
	make -C iozone clean
	make -C libc-bench clean
	make -C UnixBench clean
	make -C time-test clean
	- rm sdcard/*
	- rm sdcard.img
	- rm sdcard.img.gz

.PHONY:
