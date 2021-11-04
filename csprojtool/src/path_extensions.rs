use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf, Prefix};

#[derive(Eq, PartialEq, PartialOrd, Ord)]
enum ReducedPrefix<'a> {
    Verbatim(&'a OsStr),
    DeviceNS(&'a OsStr),
    UNC(&'a OsStr, &'a OsStr),
    Disk(u8),
}

impl<'a> From<Prefix<'a>> for ReducedPrefix<'a> {
    fn from(prefix: Prefix<'a>) -> Self {
        match prefix {
            Prefix::Verbatim(a) => Self::Verbatim(a),
            Prefix::VerbatimUNC(a, b) => Self::UNC(a, b),
            Prefix::VerbatimDisk(a) => Self::Disk(a),
            Prefix::DeviceNS(a) => Self::DeviceNS(a),
            Prefix::UNC(a, b) => Self::UNC(a, b),
            Prefix::Disk(a) => Self::Disk(a),
        }
    }
}

fn comps_eq(a: &Component, b: &Component) -> bool {
    match (a, b) {
        (Component::Prefix(a), Component::Prefix(b)) => {
            ReducedPrefix::from(a.kind()) == ReducedPrefix::from(b.kind())
        }
        _ => a == b,
    }
}

pub fn relative_path(abs_src_dir: &Path, abs_dst_path: &Path) -> PathBuf {
    let mut abs_src_dir_comps = abs_src_dir.components().peekable();
    let mut abs_dst_path_comps = abs_dst_path.components().peekable();

    // Skip common prefix
    while let (Some(sc), Some(tc)) = (abs_src_dir_comps.peek(), abs_dst_path_comps.peek()) {
        if !comps_eq(sc, tc) {
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

fn join_components<'a, I: Iterator<Item = Component<'a>>>(result: &mut PathBuf, components: I) {
    for current in components {
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
}

pub trait PathExt {
    fn simplify(&self) -> PathBuf;

    /// Prepends the current directory (working directory) if the path is not already absolute.
    fn simplified_absolute(&self) -> std::io::Result<PathBuf>;
}

impl PathExt for Path {
    fn simplify(&self) -> PathBuf {
        let mut path = PathBuf::with_capacity(self.as_os_str().len());
        join_components(&mut path, self.components());
        path
    }

    /// Prepends the current directory (working directory) if the path is not already absolute.
    fn simplified_absolute(&self) -> std::io::Result<PathBuf> {
        if self.is_absolute() {
            Ok(self.simplify())
        } else {
            let mut path = std::env::current_dir()?;
            path.reserve(path.as_os_str().len() + self.as_os_str().len());
            join_components(&mut path, self.components());
            Ok(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathbuf_only_retains_last_absolute_path() {
        assert_eq!(
            [r"\\?\C:\Users\Rust\README.md", r"\\?\D:\Data\files.txt"]
                .iter()
                .collect::<PathBuf>(),
            Path::new(r"\\?\D:\Data\files.txt"),
        );
    }

    #[test]
    fn simplify_works() {
        assert_eq!(
            Path::new(r"\\?\D:\Data\Nested\..\files.txt").simplify(),
            Path::new(r"\\?\D:\Data\files.txt"),
        );

        assert_eq!(
            Path::new(r"\\?\D:\Data\Nested\..\..\files.txt").simplify(),
            Path::new(r"\\?\D:\files.txt"),
        );
    }

    #[test]
    fn simplify_drops_curdir() {
        assert_eq!(
            Path::new(r"\\?\D:\.\files.txt").simplify(),
            Path::new(r"\\?\D:\files.txt"),
        );
    }

    #[test]
    fn simplify_retains_parent_at_root() {
        assert_eq!(
            Path::new(r"\\?\D:\..\files.txt").simplify(),
            Path::new(r"\\?\D:\..\files.txt"),
        );
    }

    #[test]
    fn relative_path_ignores_prefix_differences() {
        assert_eq!(
            PathBuf::from("Mick"),
            relative_path(Path::new(r"C:\Users"), Path::new(r"\\?\C:\Users\Mick")),
        );
    }
}
