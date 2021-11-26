use crossbeam_channel::Sender;
use ignore::ParallelVisitor;
use ignore::ParallelVisitorBuilder;
use log::debug;
use log::warn;

use crate::csproj::*;
use crate::path_extensions::*;
use crate::utils::entry_is_csproj;
use crate::utils::find_git_root;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct Options<'a> {
    pub search_path: &'a Path,
    pub follow_incoming_project_references: bool,
    pub follow_outgoing_project_references: bool,
}

pub fn run(options: Options) {
    let projects = list(options);

    let current_dir = std::env::current_dir().unwrap();
    for project in &projects {
        println!("{}", relative_path(&current_dir, &project.path).display());
    }
}

pub fn list(options: Options) -> Vec<Project> {
    let Options {
        search_path,
        follow_outgoing_project_references,
        follow_incoming_project_references,
    } = options;

    let search_path = search_path.simplified_absolute().unwrap();
    let search_meta = std::fs::metadata(&search_path).expect("Failed to get search path metadata!");

    let current_dir = std::env::current_dir().unwrap();

    let root_dir = match find_git_root(if search_meta.is_file() {
        search_path.parent().unwrap()
    } else {
        &search_path
    }) {
        Some(root_dir) => {
            debug!("Using {} as root directory.", root_dir.display());
            root_dir
        }
        None => {
            warn!(
                "No git root found, using the current directory {} as root directory.",
                current_dir.display()
            );
            &current_dir
        }
    };

    let (sender, receiver) = crossbeam_channel::unbounded();

    let mut visitor_builder = CollectorBuilder { sender };

    let walk_builder = ignore::WalkBuilder::new(root_dir);
    walk_builder.build_parallel().visit(&mut visitor_builder);

    drop(visitor_builder);

    let projects = receiver
        .into_iter()
        .flat_map(|projects| projects)
        .collect::<Vec<_>>();

    let path_to_project_index = projects
        .iter()
        .enumerate()
        .map(|(index, project)| (project.path.to_owned(), index))
        .collect::<BTreeMap<_, _>>();

    let rel_search_path = relative_path(&current_dir, &search_path);

    let mut included = projects
        .iter()
        .map(|project| {
            let rel_path = relative_path(&current_dir, &project.path);
            rel_path.starts_with(&rel_search_path)
        })
        .collect::<Vec<_>>();

    let edges: Vec<(usize, usize)> = projects
        .iter()
        .enumerate()
        .flat_map(|(from_index, project)| {
            project
                .project_references
                .iter()
                .filter_map(|to_path| {
                    if let Some(to_index) = path_to_project_index.get(to_path).copied() {
                        Some((from_index, to_index))
                    } else {
                        warn!(
                            "Reference from {} to {} not found in parsed projects under {}!",
                            project.path.display(),
                            to_path.display(),
                            root_dir.display()
                        );
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    // Follow incoming references
    if follow_incoming_project_references {
        while let Some((from, _)) = edges
            .iter()
            .copied()
            .find(|&(from, to)| !included[from] && included[to])
        {
            included[from] = true;
        }
    }

    // Follow outgoing references
    if follow_outgoing_project_references {
        while let Some((_, to)) = edges
            .iter()
            .copied()
            .find(|&(from, to)| included[from] && !included[to])
        {
            included[to] = true;
        }
    }

    projects
        .into_iter()
        .enumerate()
        .filter_map(
            |(index, project)| {
                if included[index] {
                    Some(project)
                } else {
                    None
                }
            },
        )
        .collect()
}

struct Collector {
    projects: Vec<Project>,
    sender: Sender<Vec<Project>>,
}

impl Collector {
    pub fn new(sender: Sender<Vec<Project>>) -> Self {
        Self {
            projects: Default::default(),
            sender,
        }
    }
}

impl ParallelVisitor for Collector {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
        let entry = entry.unwrap();
        if entry_is_csproj(&entry) {
            let path = std::fs::canonicalize(entry.path()).unwrap();
            match read_and_parse_project(path.clone()) {
                Ok(project) => self.projects.push(project),
                Err(e) => {
                    warn!(
                        "Ignoring project at {} due parsing failure: {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        ignore::WalkState::Continue
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        let projects = std::mem::take(&mut self.projects);
        self.sender.send(projects).unwrap();
    }
}

struct CollectorBuilder {
    sender: Sender<Vec<Project>>,
}

impl<'s> ParallelVisitorBuilder<'s> for CollectorBuilder {
    fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
        Box::new(Collector::new(self.sender.clone()))
    }
}
