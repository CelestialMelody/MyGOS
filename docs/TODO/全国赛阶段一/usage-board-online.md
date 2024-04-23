# 线上 u740 平台操作流程

1. 赛题 -> [操作系统内核实现赛-全国赛-Unmatched](https://course.educg.net/sv2/indexexp/contest/contest_submit.jsp?contestID=zrjE9c0I24Q&taskID=48111&my=false&contestCID=0) -> 点开后右上角的 `在线 IDE`

2. 点开 `在线 IDE` 后会进入 VSCode Web，通过 **Ctrl + `** 打开终端，跳转到目标文件夹

   ```sh
   # 启动开发板
   /cg/control on
   # 本地编译出 os.bin 后拖拽到 /srv/tftp/u2 中(本地文件夹拖到在线 VSCode 上)
   # 后续命令的执行需要在 /srv/tftp/ 目录下，因为传送文件时需要有个 u2/ 前缀(u2/os.bin)
   cd /srv/tftp/
   # 一般来说 minicom 没有被安装，需要先安装(这和提交测评那块的文档有出入)
   sudo apt update && sudo apt install minicom -y
   # 安装完成后使用 minicom 与 uboot 交互
   minicom -D /dev/ttyUSB1 -b 115200
   ```


### 汇总

minicom 一开始没法输入，需要更改设置：

1. Ctrl + a，松开按 O，j,k 上下移动选择 Serial port setup，回车确定
2. 单击 F，使  Hardware Flow Control 变成 No，回车，选择 Exit 回车，现在可以输入了

1. 输入

    ```Shell
        bootp 0x80200000 u2/os.bin
    ```

2.  将内核通过 tftp 发送到板上，开跑

    ```Shell
        go 0x80200000
    ```

退出并 reset：Ctrl + A，松开按 X，回车，退出 minicom