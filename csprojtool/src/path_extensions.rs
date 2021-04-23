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
        let mut stack = Vec::new();
        for current in self.components() {
            match current {
                Component::CurDir => {}
                Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                    stack.push(current)
                }
                Component::ParentDir => {
                    let should_pop = matches!(stack.last(), Some(Component::Normal(_)));
                    if should_pop {
                        stack.pop();
                    } else {
                        stack.push(current);
                    }
                }
            }
        }
        return stack.iter().collect();
    }
}
