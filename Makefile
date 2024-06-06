KERNEL_ELF = ./kernel/target/riscv64gc-unknown-none-elf/release/kernel

define hide-cargo-config
	@mv kernel/.cargo kernel/cargo
endef
define recover-cargo-config
	@mv kernel/cargo kernel/.cargo
endef
define make-kernel-submit
	@cd kernel/ && make kernel
endef

# submit 分支被拉取后由测试机自行执行该命令获取到编译后的内核
# 要求该操作生成 kernel-qemu
all:
	$(recover-cargo-config)
	$(make-kernel-submit)
	@cp $(KERNEL_ELF) ./kernel-qemu

# 将当前分支强制更新到提交分支
submit:
	@echo "Prepare .cargo dir..."
	$(hide-cargo-config)
	@echo "Delete remote submit branch, pushing current as the new one..."
	@git push origin :submit
	@git push origin comp-final-qemu:submit
	@echo "Recover .cargo dir..."
	$(recover-cargo-config)

run:
	@cd kernel/ && make run
debug-server:
	@cd kernel/ && make debug-server
debug:
	@cd kernel/ && make debug

init: sdcard

sdcard:
	@cd testsuits/ \
	&& docker run --rm -it -v $$(pwd):/code --privileged --entrypoint make alphamj/os-contest:v7.7 -C /code sdcard \
	&& mv sdcard.img ../workspace/sdcard.img.bak \
	&& cp ../workspace/sdcard.img.bak ../workspace/sdcard.img
	@echo 'sdcard.img, sdcard.img.bak have been created successfully! You are ready to go :-)'

# 使用 Dockerfile 构建的镜像名
DOCKER_IMAGE_NAME := kernel-dev-env
# 使用 docker run 启动的容器名
CONTAINER_NAME := os-env-test
# 将主机工作目录挂载到容器内的 /mnt 目录
WORKPLACE := /mnt/d/wsl/comp
# 主机端口
HOST_PORT := 9090
# 容器端口
CONTAINER_PORT := 9090
# 使用 Dockerfile 构建镜像
build_docker:
	docker build -t ${DOCKER_IMAGE_NAME} .
# 使用 Dockerfile 创建并启动容器
run_docker:
	docker run -p ${HOST_PORT}:${CONTAINER_PORT} --name ${CONTAINER_NAME} -v ${WORKPLACE}:/mnt -w /mnt -it -d ${DOCKER_IMAGE_NAME} bash
# 进入容器
exec_docker:
	docker exec -it ${CONTAINER_NAME} bash

clean:
	@rm -f kernel-qemu
	@cd kernel/ && cargo clean
	@cd workspace/ && make clean
	@cd crates/libd && cargo clean
	@cd crates/sync_cell && cargo clean
	@cd crates/fat32 && cargo clean
	@cd testsuits/ && make clean
	@echo 'All cleaned, if you want to run the kernel, do make sdcard first :-)'

.PHONY: all clean sdcard init debug-server debug submit
