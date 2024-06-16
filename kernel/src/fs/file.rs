use crate::fs::ino_alloc;
use crate::fs::File;
#[cfg(feature = "fat32")]
use crate::fs::Inode;
#[cfg(feature = "fat32")]
use crate::fs::KFile;
#[cfg(feature = "fat32")]
use crate::fs::INODE_CACHE;
use crate::return_errno;
use crate::syscall::impls::Errno;
use crate::BLOCK_DEVICE;
use alloc::string::ToString;
use alloc::sync::Arc;
use fat32::{root, Dir, DirError, FileSystem, VirtFile, VirtFileType, ATTR_DIRECTORY};
use nix::{CreateMode, OpenFlags};
pub use path::*;
use spin::lazy::Lazy;
use spin::rwlock::RwLock;
use spin::Mutex;

#[cfg(feature = "ramfs")]
use crate::fs::MountedInfo;
#[cfg(feature = "ramfs")]
use crate::fs::RamDir;
#[cfg(feature = "ramfs")]
use crate::fs::RamFs;

// pub use mount::MNT_TABLE;
// pub use pipe::{make_pipe, Pipe};
// pub use stdio::{Stdin, Stdout};

#[cfg(feature = "ramfs")]
pub static FILE_SYSTEM: Lazy<Arc<RwLock<RamFs>>> =
    Lazy::new(|| Arc::new(RwLock::new(RamFs::new())));

#[cfg(feature = "ramfs")]
pub static ROOT_INODE: Lazy<Arc<RamDir>> = Lazy::new(|| {
    let fs = FILE_SYSTEM.read();
    let root_dir = fs.root_dir(MountedInfo {
        fs_id: 0,
        path: AbsolutePath::from_str("/"),
    });
    root_dir
});

#[cfg(feature = "ramfs")]
pub fn open(
    path: AbsolutePath,
    flags: OpenFlags,
    _mode: CreateMode,
) -> Result<Arc<dyn File>, Errno> {
    if path == AbsolutePath::from_str("/") {
        return Ok(ROOT_INODE.clone());
    }
    let pathv = path.as_vec_str();
    let target = pathv.last().unwrap();
    match find_parent_dir(path.clone()) {
        Some(parent) => match parent.open(target) {
            Some(file) => {
                let res = file.to_inode(MountedInfo { fs_id: 0, path });
                res.set_flags(flags);
                Ok(res)
            }
            None => {
                if flags.contains(OpenFlags::O_CREAT) {
                    if flags.contains(OpenFlags::O_DIRECTORY) {
                        parent.mkdir(target, flags)
                    } else {
                        parent.touch(target, flags)
                    }
                } else {
                    Err(Errno::ENOENT)
                }
            }
        },
        None => Err(Errno::ENOENT),
    }
}

#[cfg(feature = "ramfs")]
pub fn chdir(path: AbsolutePath) -> bool {
    if path == AbsolutePath::from_str("/") {
        return true;
    }
    let pathv = path.as_vec_str();
    let target = pathv.last().unwrap();
    match find_parent_dir(path.clone()) {
        Some(parent) => parent.open(target).is_some(),
        None => false,
    }
}

#[cfg(feature = "ramfs")]
pub fn find_parent_dir(path: AbsolutePath) -> Option<Arc<RamDir>> {
    let mut current_dir = ROOT_INODE.clone();
    if path == AbsolutePath::from_str("/") {
        return Some(current_dir);
    }
    let pathv = path.as_vec_str();
    // 找到父目录
    for name in pathv.iter().take(pathv.len() - 1) {
        match current_dir.open_dir(name) {
            Some(dir) => {
                current_dir = dir;
            }
            None => return None,
        }
    }
    Some(current_dir)
}

#[cfg(feature = "fat32")]
pub static FILE_SYSTEM: Lazy<Arc<RwLock<FileSystem>>> = Lazy::new(|| {
    let blk = BLOCK_DEVICE.clone();
    FileSystem::open(blk)
});

#[cfg(feature = "fat32")]
pub static ROOT_INODE: Lazy<Arc<VirtFile>> = Lazy::new(|| {
    let fs = FILE_SYSTEM.clone();
    Arc::new(root(fs))
});

#[cfg(feature = "fat32")]
pub fn open(path: AbsolutePath, flags: OpenFlags, _mode: CreateMode) -> Result<Arc<KFile>, Errno> {
    #[cfg(feature = "time-tracer")]
    time_trace!("open");
    let (readable, writable) = flags.read_write();
    let mut pathv = path.as_vec_str();

    // println!("open test 0.1");

    #[cfg(not(feature = "no-page-cache"))]
    if let Some(inode) = INODE_CACHE.get(&path) {
        // println!("open test 0.1.1");
        let name = if let Some(name_) = pathv.last() {
            name_.to_string()
        } else {
            "/".to_string()
        };
        let res = Arc::new(KFile::new(
            readable,
            writable,
            inode.clone(),
            path.clone(),
            name,
        ));

        // println!("open test 0.1.2");

        res.create_page_cache_if_needed();

        res.set_flags(flags);

        // println!("open test 0.1.3");

        return Ok(res);
    }

    // println!("open test 0.2");

    if flags.contains(OpenFlags::O_CREAT) {
        // Create File
        let res = ROOT_INODE.find(pathv.clone());

        // println!("open test 0.3");

        match res {
            Ok(file) => {
                // println!("open test 0.4");

                let name = if let Some(name_) = pathv.pop() {
                    name_
                } else {
                    "/"
                };
                let fid = ino_alloc();
                #[cfg(not(feature = "no-page-cache"))]
                let file_size = file.file_size();
                #[cfg(not(feature = "no-page-cache"))]
                let inode = Arc::new(Inode {
                    file: Mutex::new(file),
                    fid,
                    page_cache: Mutex::new(None),
                    file_size: Mutex::new(file_size),
                });
                #[cfg(feature = "no-page-cache")]
                let inode = Arc::new(Inode {
                    fid,
                    file: Mutex::new(file),
                });

                let res = Arc::new(KFile::new(
                    readable,
                    writable,
                    inode.clone(),
                    path.clone(),
                    name.to_string(),
                ));

                // println!("open test 0.5");

                // create page cache
                #[cfg(not(feature = "no-page-cache"))]
                res.create_page_cache_if_needed();

                // println!("open test 0.6");

                #[cfg(not(feature = "no-page-cache"))]
                INODE_CACHE.insert(path.clone(), inode.clone());

                // println!("open test 0.7");
                Ok(res)
            }
            Err(_err) => {
                // println!("open test 0.8");

                if _err == DirError::NotDir {
                    return Err(Errno::ENOTDIR);
                }
                let mut create_type = VirtFileType::File;
                if flags.contains(OpenFlags::O_DIRECTORY) {
                    create_type = VirtFileType::Dir;
                }

                // to find parent
                let name = pathv.pop().unwrap();

                // println!("open test 0.9");

                match ROOT_INODE.find(pathv.clone()) {
                    // find parent to create file
                    Ok(parent) => match parent.create(name, create_type as VirtFileType) {
                        Ok(file) => {
                            // println!("open test 0.10");

                            let fid = ino_alloc();
                            #[cfg(not(feature = "no-page-cache"))]
                            let file_size = file.file_size();
                            #[cfg(not(feature = "no-page-cache"))]
                            let inode = Arc::new(Inode {
                                file: Mutex::new(Arc::new(file)),
                                fid,
                                page_cache: Mutex::new(None),
                                file_size: Mutex::new(file_size),
                            });
                            #[cfg(feature = "no-page-cache")]
                            let inode = Arc::new(Inode {
                                fid,
                                file: Mutex::new(Arc::new(file)),
                            });
                            let res = Arc::new(KFile::new(
                                readable,
                                writable,
                                inode.clone(),
                                path.clone(),
                                name.to_string(),
                            ));
                            #[cfg(not(feature = "no-page-cache"))]
                            res.create_page_cache_if_needed();
                            #[cfg(not(feature = "no-page-cache"))]
                            INODE_CACHE.insert(path.clone(), inode.clone());
                            Ok(res)
                        }
                        Err(_err) => Err(Errno::DISCARD),
                    },
                    Err(_err) => {
                        return_errno!(Errno::ENOENT, "parent path not exist path:{:?}", path)
                    }
                }
            }
        }
    } else {
        // Open File
        match ROOT_INODE.find(pathv.clone()) {
            Ok(file) => {
                // println!("open test 0.11");

                // clear file if O_TRUNC
                if flags.contains(OpenFlags::O_TRUNC) {
                    file.clear();
                }
                let name = file.name().to_string();
                let fid = ino_alloc();
                #[cfg(not(feature = "no-page-cache"))]
                let file_size = file.file_size();
                #[cfg(not(feature = "no-page-cache"))]
                let inode = Arc::new(Inode {
                    file: Mutex::new(file),
                    fid,
                    file_size: Mutex::new(file_size),
                    page_cache: Mutex::new(None),
                });
                #[cfg(feature = "no-page-cache")]
                let inode = Arc::new(Inode {
                    fid,
                    file: Mutex::new(file),
                });
                let res = Arc::new(KFile::new(
                    readable,
                    writable,
                    inode.clone(),
                    path.clone(),
                    name,
                ));
                #[cfg(not(feature = "no-page-cache"))]
                res.create_page_cache_if_needed();
                #[cfg(not(feature = "no-page-cache"))]
                INODE_CACHE.insert(path.clone(), inode.clone());
                res.set_flags(flags);
                Ok(res)
            }
            Err(_err) => return_errno!(Errno::ENOENT, "no such file or path:{:?}", path),
        }
    }
}

// TODO This only used to check whether can cd to path
#[cfg(feature = "fat32")]
pub fn chdir(path: AbsolutePath) -> bool {
    if let Ok(_) = ROOT_INODE.find(path.as_vec_str()) {
        true
    } else {
        false
    }
}

#[cfg(feature = "fat32")]
pub fn list_apps(path: AbsolutePath) {
    let layer: usize = 0;
    fn ls(path: AbsolutePath, layer: usize) {
        let dir = ROOT_INODE.find(path.as_vec_str()).unwrap();
        println!("dir name: {:?}", dir.name());

        for app in dir.ls_with_attr().unwrap() {
            // no print initproc(However, it is deleted after task::new)
            if layer == 0 && app.0 == "initproc" {
                continue;
            }
            let app_path: AbsolutePath = path.cd(app.0.clone());
            if app.1 & ATTR_DIRECTORY == 0 {
                // if it is not directory
                for _ in 0..layer {
                    crate::print!("    ");
                }
                crate::println!("{}", app.0);
            } else if app.0 != "." && app.0 != ".." {
                // if it is directory
                for _ in 0..layer {
                    crate::print!("    ");
                }
                crate::println!("{}/", app.0);
                ls(app_path.clone(), layer + 1);
            }
        }
    }

    ls(path, layer);
}
