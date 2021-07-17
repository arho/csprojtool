use std::path::{Component, Path, PathBuf};

pub fn relative_path(abs_src_dir: &Path, abs_dst_path: &Path) -> PathBuf {
    let mut abs_src_dir_comps = abs_src_dir.components().peekable();
    let mut abs_dst_path_comps = abs_dst_path.components().peekable();

    // Skip common prefix
    while let (Some(sc), Some(tc)) = (abs_src_dir_comps.peek(), abs_dst_path_comps.peek()) {
        if sc != tc {
            break;
        }
        abs_src_dir_comps.next();
        abs_dst_path_comps.next();
    }

    abs_src_dir_comps
        .map(|_| Component::ParentDir)
        .chain(abs_dst_path_comps)
        .collect()
}

pub trait PathExt {
    fn simplify(&self) -> PathBuf;
}

impl PathExt for Path {
    fn simplify(&self) -> PathBuf {
        let mut result = PathBuf::with_capacity(self.as_os_str().len());

        for current in self.components() {
            match current {
                Component::CurDir => {}
                Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                    result.push(current);
                }
                Component::ParentDir => {
                    if matches!(result.components().last(), Some(Component::Normal(_))) {
                        result.pop();
                    } else {
                        result.push(current);
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathbuf_only_retains_last_absolute_path() {
        assert_eq!(
            Path::new(r"\\?\D:\Data\files.txt"),
            [r"\\?\C:\Users\Rust\README.md", r"\\?\D:\Data\files.txt"]
                .iter()
                .collect::<PathBuf>()
        );
    }

    #[test]
    fn simplify_works() {
        assert_eq!(
            Path::new(r"\\?\D:\Data\files.txt"),
            Path::new(r"\\?\D:\Data\Nested\..\files.txt").simplify(),
        );

        assert_eq!(
            Path::new(r"\\?\D:\files.txt"),
            Path::new(r"\\?\D:\Data\Nested\..\..\files.txt").simplify(),
        );
    }

    #[test]
    fn simplify_drops_curdir() {
        assert_eq!(
            Path::new(r"\\?\D:\files.txt"),
            Path::new(r"\\?\D:\.\files.txt").simplify(),
        );
    }

    #[test]
    fn simplify_retains_parent_at_root() {
        assert_eq!(
            Path::new(r"\\?\D:\..\files.txt"),
            Path::new(r"\\?\D:\..\files.txt").simplify(),
        );
    }
}
