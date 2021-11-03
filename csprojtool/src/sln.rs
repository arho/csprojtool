mod file;
mod types;

use log::debug;

use crate::csproj::*;
use crate::path_extensions::*;
use std::path::Path;
use std::path::PathBuf;

pub struct SlnOptions {
    pub sln_path: PathBuf,
    pub search_path: PathBuf,
    pub glob_matcher: globset::GlobMatcher,
    pub follow_project_references: bool,
}

pub fn sln(options: SlnOptions) {
    let SlnOptions {
        sln_path,
        search_path,
        glob_matcher,
        follow_project_references,
    } = options;

    debug!(
        "Generating solution {} starting in {} matching {}{}.",
        sln_path.display(),
        search_path.display(),
        glob_matcher.glob(),
        if follow_project_references {
            " while following project references"
        } else {
            " without following project references"
        },
    );

    let projects = parse_projects(&search_path, &glob_matcher, follow_project_references);

    let sln = create_solution(&sln_path, projects.into_iter().map(|(_, p)| p.unwrap()));

    let file = std::fs::File::create(&sln_path).unwrap();
    let mut writer = std::io::BufWriter::new(file);
    sln.write(&mut writer).unwrap();
}

fn create_solution(sln_path: &Path, projects: impl Iterator<Item = Project>) -> file::SolutionFile {
    let mut root = file::Directory::default();
    let sln_path = sln_path.ensure_absolute().unwrap().simplify();
    let sln_dir = sln_path.parent().unwrap();

    for project in projects {
        let rel_project_path = relative_path(sln_dir, &project.path);

        debug!("Adding {}", rel_project_path.display());

        let mut components = rel_project_path.components().peekable();

        let mut dir = &mut root;
        while let Some(comp) = components.next() {
            let comp = match comp {
                std::path::Component::ParentDir => {
                    panic!("Can not reference projects outside of solution directory!")
                }
                std::path::Component::Normal(val) => val.to_str().unwrap().to_owned(),
                _ => panic!("Unexpected path component!"),
            };

            if components.peek().is_some() {
                dir = match dir
                    .nodes
                    .entry(comp)
                    .or_insert_with(|| file::Node::Directory(file::Directory::default()))
                {
                    file::Node::Directory(dir) => dir,
                    file::Node::Project(_) => panic!("Project path used as directory!"),
                };
            } else {
                dir.nodes.insert(
                    comp,
                    file::Node::Project(file::Project {
                        guid: project.project_guid,
                    }),
                );
            }
        }
    }

    file::SolutionFile::new(root)
}
