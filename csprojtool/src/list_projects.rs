use crate::csproj::*;
use crate::path_extensions::*;
use std::path::PathBuf;

pub struct ListProjectsOptions {
    pub search_path: PathBuf,
    pub glob_matcher: globset::GlobMatcher,
    pub follow_project_references: bool,
    pub exclude_sdk: bool,
}

pub fn list_projects(options: &ListProjectsOptions) {
    let ListProjectsOptions {
        ref search_path,
        ref glob_matcher,
        follow_project_references,
        exclude_sdk,
    } = *options;

    let walk_builder = ignore::WalkBuilder::new(search_path);
    let cwd = std::env::current_dir().unwrap();
    walk_builder.build_parallel().run(|| {
        Box::new(move |entry| {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            if meta.is_file() && glob_matcher.is_match(entry.path()) {
                println!("{}", entry.path().display());
            }

            ignore::WalkState::Continue
        })
    });

    // let projects = parse_projects(search_path, glob_matcher, follow_project_references);

    // let cwd = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    // let mut project_paths = projects
    //     .into_iter()
    //     .map(|(path, project)| (relative_path(&cwd, path.as_path()), project.unwrap()))
    //     .collect::<Vec<_>>();

    // project_paths.sort_by(|(a, _), (b, _)| a.cmp(b));

    // for (path, project) in project_paths {
    //     if exclude_sdk && project.is_sdk {
    //         continue;
    //     }

    //     println!("{}", path.display())
    // }
}
