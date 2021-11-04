use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub fn find_git_root(dir: &Path) -> Option<&Path> {
    dir.ancestors().find(|&dir| dir_contains_git(dir))
}

fn dir_contains_git(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .unwrap()
        .any(|entry| entry_is_git(&entry.unwrap()))
}

fn entry_is_git(entry: &std::fs::DirEntry) -> bool {
    entry.file_type().unwrap().is_dir() && entry.file_name() == ".git"
}

pub fn find_dir_csproj(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir).unwrap().filter_map(|entry| {
        let entry = entry.unwrap();
        if std_entry_is_csproj(&entry) {
            Some(entry.path())
        } else {
            None
        }
    })
}

pub fn path_extension_is_csproj(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("csproj"))
}

fn std_entry_is_csproj(entry: &std::fs::DirEntry) -> bool {
    entry.file_type().unwrap().is_file() && path_extension_is_csproj(entry.file_name().as_ref())
}

pub fn entry_is_csproj(entry: &ignore::DirEntry) -> bool {
    entry.file_type().unwrap().is_file() && path_extension_is_csproj(entry.file_name().as_ref())
}
