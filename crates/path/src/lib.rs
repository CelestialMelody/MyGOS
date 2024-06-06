#![no_std]

extern crate alloc;

use alloc::format;
use alloc::vec::Vec;
use alloc::{collections::VecDeque, string::String};
use core::fmt::{Debug, Formatter};
use core::ops::{Deref, DerefMut};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbsolutePath {
    pub components: VecDeque<String>,
}

impl From<&str> for AbsolutePath {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl From<String> for AbsolutePath {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl From<Vec<&str>> for AbsolutePath {
    fn from(s: Vec<&str>) -> Self {
        Self::from_vec_str(s)
    }
}

impl Deref for AbsolutePath {
    type Target = VecDeque<String>;

    fn deref(&self) -> &Self::Target {
        &self.components
    }
}
impl DerefMut for AbsolutePath {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.components
    }
}
impl AbsolutePath {
    fn from<T>(s: T) -> Self
    // T 可迭代必须实现 IntoIterator trait
    // T::Item 可转换为字符串引用, 必须实现 AsRef<str> trait 与 PartialEq<&'static str> trait
    // T::Item 可以将其转换为 String, 必须实现 Into<String> trait
    where
        T: IntoIterator,
        <T as IntoIterator>::Item: AsRef<str> + PartialEq<&'static str>,
        <T as IntoIterator>::Item: Into<String>,
    {
        let mut components = VecDeque::new();
        for name in s {
            if name == "." || name == "" {
                continue;
            } else if name == ".." {
                let ret = components.pop_back();
                if ret.is_none() {
                    components.push_back(name.into());
                }
            } else {
                components.push_back(name.into());
            }
        }
        Self { components }
    }
}
impl AbsolutePath {
    pub fn from_string(path: String) -> Self {
        let temp: VecDeque<String> = path.split('/').map(|s| String::from(s)).collect();
        Self::from(temp)
    }
    pub fn from_str(str: &str) -> AbsolutePath {
        AbsolutePath::from_string(String::from(str))
    }
    pub fn from_vec_str(vec: Vec<&str>) -> Self {
        Self::from(vec)
    }
    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
    pub fn as_vec_str(&self) -> Vec<&str> {
        self.components.iter().map(|s| s.as_str()).collect()
    }
    /// Whether it is the root
    pub fn is_root(&self) -> bool {
        return self.components.len() == 0;
    }
    /// Get the tail of the path
    pub fn last(&self) -> String {
        if self.is_root() {
            return String::from("/");
        }
        return self.components[self.len() - 1].clone();
    }
    pub fn name(&self) -> String {
        self.last()
    }
    #[allow(unused)]
    pub fn first(&self) -> String {
        return self.components[0].clone();
    }
    pub fn index(&self, index: usize) -> String {
        self.components[index].clone()
    }
    pub fn parent(&self) -> Self {
        if self.is_root() {
            panic!("already root")
        }
        let mut new = self.clone();
        new.pop_back();
        new
    }
    pub fn layer(&self) -> usize {
        self.components.len()
    }
    pub fn remove_prefix(&self, prefix: &AbsolutePath) -> Self {
        assert!(self.start_with(prefix), "not prefix");
        let mut new = self.clone();
        for _ in 0..prefix.len() {
            new.pop_front();
        }
        new
    }
    /// Whether it is started with the prefix
    pub fn start_with(&self, prefix: &AbsolutePath) -> bool {
        if prefix.is_root() {
            return true;
        }
        if prefix.len() > self.len() {
            return false;
        }
        for (this_i, pre_i) in self.components.iter().zip(prefix.components.iter()) {
            if this_i != pre_i {
                return false;
            }
        }
        true
    }
    /// 根据传入的 path 拼接成新的 absolute_path 并返回, 不改变原有的 absolute_path;
    /// 如果 path 是绝对路径, 则返回 path; 若为相对路径, 则将 path 拼接到 self 后面
    pub fn cd(&self, path: String) -> Self {
        if path.starts_with('/') {
            return AbsolutePath::from_string(path.clone());
        }
        let mut res = self.clone();
        let path = AbsolutePath::from_string(path.clone());
        for p in path.components.into_iter() {
            res.push_back(p);
        }
        res
    }
}
impl Debug for AbsolutePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        if self.is_root() {
            write!(f, "/")?;
            return Ok(());
        }
        for p in self.components.iter() {
            write!(f, "/{}", p)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path() {
        let path = AbsolutePath::from_string(String::from("/a/b/c/d/"));
        assert_eq!(format!("{:?}", path), "/a/b/c/d/");
        let path = AbsolutePath::from_string(String::from("/abcdefg/asdsd/asdasd"));
        assert_eq!(format!("{:?}", path), "/abcdefg/asdsd/asdasd");
        let path = AbsolutePath::from_string(String::from("aa/../bb/../cc/././."));
        assert_eq!(format!("{:?}", path), "/cc");
        let path = AbsolutePath::from_string(String::from("aa/../.."));
        assert_eq!(format!("{:?}", path), "/");
        let path = AbsolutePath::from_string(String::from("./././."));
        assert_eq!(format!("{:?}", path), "/");

        // test cd
        let abs_path = AbsolutePath::from_string(String::from("/a/b/c/d/"));
        let path = String::from("../e/../f/g");
        let new_path = abs_path.cd(path);
        assert_eq!(format!("{:?}", new_path), "/a/b/c/f/g");

        // test join
        let abs_path = AbsolutePath::from_string(String::from("/a/b/c/d/"));
        let new_path = abs_path.cd(String::from("../e/../f/g"));
        assert_eq!(format!("{:?}", new_path), "/a/b/c/f/g");
    }
}
