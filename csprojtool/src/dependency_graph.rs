use crate::csproj::*;
use crate::path_extensions::*;
use std::{collections::HashMap, path::PathBuf};

pub fn dependency_graph(glob: &str, search: &str, dot: Option<&str>, json: Option<&str>) {
    // if we pass a file path, projects should contain that file
    // if we pass a directory path, projects should glob that directory
    // if we don't pass a path, projects should glob the current directory

    let (search_dir, mut projects) = {
        let search_path =
            std::fs::canonicalize(search).expect("Failed to canonicalize path, does it exist?");
        let meta = std::fs::metadata(&search_path).unwrap();
        if meta.is_file() {
            let search_dir = search_path.parent().unwrap().to_path_buf();
            let projects = Some((search_path, None)).into_iter().collect();
            (search_dir, projects)
        } else if meta.is_dir() {
            let search_dir = search_path;
            let original_current_dir = std::env::current_dir().unwrap();
            std::env::set_current_dir(&search_dir).unwrap();
            let projects = search_for_projects(glob);
            std::env::set_current_dir(original_current_dir).unwrap();
            (search_dir, projects)
        } else {
            panic!("Specified path is not a file nor a directory!")
        }
    };

    loop {
        let todo = projects
            .iter()
            .filter_map(|(path, project)| {
                if project.is_some() {
                    None
                } else {
                    Some(path.clone())
                }
            })
            .collect::<Vec<_>>();

        if todo.is_empty() {
            break;
        }

        for project_path in todo {
            let project = read_and_parse_project(project_path.clone());

            if let Ok(project) = &project {
                for path in project.project_references.iter().cloned() {
                    projects.entry(path).or_insert(None);
                }
            }

            projects.insert(project_path, Some(project));
        }
    }

    let mut projects = projects
        .into_iter()
        .map(|(project_path, maybe_project)| {
            let project_path = relative_path(&search_dir, &project_path);

            let mut project = maybe_project.unwrap();
            if let Ok(project) = project.as_mut() {
                project.path = relative_path(&search_dir, &project.path);
                for dependency_path in project.project_references.iter_mut() {
                    *dependency_path = relative_path(&search_dir, dependency_path);
                }
            }
            (project_path, project)
        })
        .collect::<Vec<_>>();
    projects.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some(path) = dot {
        let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        serialize_dot(&mut file, &projects).unwrap();
    }

    if let Some(path) = json {
        let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        let root = JsonRoot {
            projects: projects
                .iter()
                .filter_map(|(_, project)| project.as_ref().ok().cloned())
                .collect(),
        };
        serde_json::to_writer_pretty(&mut file, &root).unwrap();
    }
}

fn serialize_dot<W: std::io::Write>(
    writer: &mut W,
    projects: &[(PathBuf, Result<Project, Error>)],
) -> std::io::Result<()> {
    writeln!(writer, "// {} projects", projects.iter().len())?;

    for (path, project) in projects.iter() {
        if let Err(e) = project {
            writeln!(
                writer,
                "// failed to read and parse {}: {}",
                path.display().to_string().replace("\\", "\\\\"),
                e
            )?;
        }
    }

    let nodes = projects
        .iter()
        .enumerate()
        .map(|(index, (path, _))| (path.clone(), index))
        .collect::<HashMap<PathBuf, usize>>();

    let edges = projects
        .iter()
        .enumerate()
        .map(|(index, (_, project))| {
            let dependencies = match project {
                Ok(project) => project
                    .project_references
                    .iter()
                    .map(|path| *nodes.get(path).unwrap())
                    .collect(),
                Err(_) => Vec::new(),
            };
            (index, dependencies)
        })
        .collect::<Vec<(usize, Vec<usize>)>>();

    writeln!(writer, "digraph {{")?;
    writeln!(writer, "  rankdir = \"LR\";")?;

    for (index, (path, project)) in projects.iter().enumerate() {
        let path_display = path.display().to_string();
        let mut parts = path_display.split("\\").peekable();
        let mut label = String::new();
        while let Some(part) = parts.next() {
            let is_last = parts.peek().is_none();
            if is_last {
                label.push_str("<B>");
            }
            label.push_str(part);
            if is_last {
                label.push_str("</B>");
            } else {
                label.push_str("<BR/>");
            }
        }

        writeln!(
            writer,
            "  n{} [label = < {} >, fillcolor = \"{}\", style = filled, shape = \"{}\"]",
            index,
            label,
            project
                .as_ref()
                .map(|project| if project.is_sdk { "#7fc79f" } else { "#fdc086" })
                .unwrap_or("red"),
            project
                .as_ref()
                .map(|project| if project.is_exe { "box" } else { "ellipse" })
                .unwrap_or("star")
        )?;
    }

    // Floyd-warshall our way to a N*N longest path matrix
    #[allow(non_snake_case)]
    let N = nodes.len();
    let mut mat: Vec<_> = std::iter::repeat(0).take(N * N).collect();

    for (i, _) in nodes.iter().enumerate() {
        mat[i * N + i] = 1;
    }

    for (source, targets) in edges.iter() {
        for target in targets {
            mat[source * N + target] = 2;
        }
    }

    for k in 0..N {
        for i in 0..N {
            for j in 0..N {
                let ik = mat[i * N + k];
                let kj = mat[k * N + j];
                let ij = mat[i * N + j];
                if ik != 0 && kj != 0 && ik + kj - 1 > ij {
                    mat[i * N + j] = ik + kj - 1
                }
            }
        }
    }

    for (source, targets) in edges {
        for target in targets {
            let longest_path = mat[source * N + target] - 1;
            if longest_path > 1 {
                writeln!(
                    writer,
                    "  n{} -> n{} [color = \"#e2e2e2\"];",
                    source, target
                )
                .unwrap();
            } else {
                writeln!(writer, "  n{} -> n{};", source, target).unwrap();
            }
        }
    }

    writeln!(writer, "}}")?;

    Ok(())
}
