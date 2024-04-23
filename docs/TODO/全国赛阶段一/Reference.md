# Reference

1. [与比赛相关的一些硬件，OS相关的实例/教程的参考信息](https://github.com/oscomp/os-competition-info/blob/main/ref-info.md)
2. [Linux kernel system calls for all architectures](https://marcin.juszkiewicz.com.pl/download/tables/syscalls.html)
3. [U740 手册](https://sifive.cdn.prismic.io/sifive/1a82e600-1f93-4f41-b2d8-86ed8b16acba_fu740-c000-manual-v1p6.pdf) 
4. [Linux 对于 u740 SPI 的适配](https://elixir.bootlin.com/linux/latest/source/drivers/spi/spi-sifive.c)



##### SDCard 信息(SDHC)

Device: spi@10050000:mmc@0                                                                                                               

Manufacturer ID: 2                                                                                                                        

OEM: 544d                                                                                                                                 

Name: SA16G                                                                                                                               

Bus Speed: 20000000                                                                                                                       

Mode: MMC legacy                                                                                                                          

Rd Block Len: 512                                                                                                                         

SD version 2.0                                                                                                                            

High Capacity: Yes                                                                                                                        

Capacity: 14.4 GiB                                                                                                                        

Bus Width: 1-bit                                                                                                                          

Erase Group Size: 512 Bytes

##### Uboot 手册

?         - alias for 'help'                                                                                                              

base      - print or set address offset                                                                                                   

bdinfo    - print Board Info structure                                                                                                    

blkcache  - block cache diagnostics and control               

boot      - boot default, i.e., run 'bootcmd'                                                                                             

bootd     - boot default, i.e., run 'bootcmd'                                                                                             

bootefi   - Boots an EFI payload from memory         

bootelf   - Boot from an ELF image in memory         

booti     - boot Linux kernel 'Image' format from memory         

bootm     - boot application image from memory                  

bootp     - boot image via network using BOOTP/TFTP protocol            

bootvx    - Boot vxWorks from an ELF image                               

cmp       - memory compare                                                                                                                

coninfo   - print console devices and information          

cp        - memory copy                                                                                                                   

cpu       - display information about CPUs                                                                                                

crc32     - checksum calculation                                                                                                          

dhcp      - boot image via network using DHCP/TFTP protocol                  

dm        - Driver model low level access                                                                                                 

echo      - echo args to console                                                                                                          

editenv   - edit environment variable                                                                                                     

eeprom    - EEPROM sub-system                                                                                                             

env       - environment handling commands                       

erase     - erase FLASH memory                                                                                                            

exit      - exit script                                                                                                                   

ext2load  - load binary file from a Ext2 filesystem          

ext2ls    - list files in a directory (default /)                                                                                         

ext4load  - load binary file from a Ext4 filesystem              

ext4ls    - list files in a directory (default /)                                                                                         

ext4size  - determine a file's size                                                                                                       

false     - do nothing, unsuccessfully                                                                                                    

fatinfo   - print information about filesystem                                                                                            

fatload   - load binary file from a dos filesystem                                                                                        

fatls     - list files in a directory (default /)                                                                                         

fatmkdir  - create a directory                                                                                                            

fatrm     - delete a file                                                                                                                 

fatsize   - determine a file's size                                                                                                       

fatwrite  - write file into a dos filesystem                                                                                              

fdt       - flattened device tree utility commands                                                                                        

flinfo    - print FLASH memory information                                                                                                

fstype    - Look up a filesystem type                                                                                                     

fstypes   - List supported filesystem types                                                                                               

go        - start application at address 'addr'                                                                                           

gpio      - query and control gpio pins                                                                                                   

gpt       - GUID Partition Table                                                                                                          

gzwrite   - unzip and write memory to block device                  

help      - print command description/usage                    

i2c       - I2C sub-system                                                                                                                

iminfo    - print header information for application image                   

imxtract  - extract a part of a multi-image                                                                                               

itest     - return true/false on integer compare                                                                                          

ln        - Create a symbolic link                                                                                                        

load      - load binary file from a filesystem                                                                                            

loadb     - load binary file over serial line (kermit mode)                 

loads     - load S-Record file over serial line                                                                                           

loadx     - load binary file over serial line (xmodem mode)                  

loady     - load binary file over serial line (ymodem mode)              

loop      - infinite loop on address range                                                                                                

ls        - list files in a directory (default /)                                                                                         

lzmadec   - lzma uncompress a memory region                    

mac       - display and program the system ID and MAC addresses in EEPROM             

md        - memory display                                                                                                                

mdio      - MDIO utility commands                                                                                                         

meminfo   - display memory information            

mii       - MII utility commands                                                                                                          

mm        - memory modify (auto-incrementing address)          

mmc       - MMC sub system                                                                                                                

mmcinfo   - display MMC info                                                                                                              

mw        - memory write (fill)                                                                                                           

net       - NET sub-system                                                                                                                

nfs       - boot image via network using NFS protocol       

nm        - memory modify (constant address)         

nvme      - NVM Express sub-system                                                                                                        

panic     - Panic with optional message                                                                                                   

part      - disk partition related commands                                                                                               

pci       - list and access PCI Configuration Space                                                                                       

ping      - send ICMP ECHO_REQUEST to network host      

printenv  - print environment variables                                                                                                   

protect   - enable or disable FLASH write protection    

pwm       - control pwm channels                                                                                                          

pxe       - commands to get and boot from pxe files     

random    - fill memory with random pattern

reset     - Perform RESET of the CPU                                                                                                      

run       - run commands in an environment variable                                                                                  

save      - save file to a filesystem                                                                                                     

saveenv   - save environment variables to persistent storage                                                                       

scsi      - SCSI sub-system                                                                                                               

scsiboot  - boot from SCSI device                                                                                                         

setenv    - set environment variables                                                                                                     

setexpr   - set environment variable as the result of eval expression  

sf        - SPI flash sub-system                                                                                                          

showvar   - print local hushshell variables                                                                                               

size      - determine a file's size                                                                                                       

sleep     - delay execution for some time                                                                                                 

source    - run script from memory                                                                                                        

sysboot   - command to get and boot from syslinux files                                                                             

test      - minimal test like /bin/sh                                                                                                     

tftpboot  - boot image via network using TFTP protocol                                                                              

true      - do nothing, successfully                                                                                                      

unlz4     - lz4 uncompress a memory region                                                                                              

unzip     - unzip a memory region                                                                                                         

usb       - USB sub-system                                                                                                                

usbboot   - boot from USB device                                                                                                          

version   - print monitor, compiler and linker version      