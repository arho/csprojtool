mod cli;
mod path_extensions;

use path_extensions::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

fn main() {
    let app = cli::build_cli();
    let matches = app.get_matches();

    if let Some(matches) = matches.subcommand_matches(cli::CMD_DEPENDENCY_GRAPH) {
        let glob = matches.value_of(cli::ARG_GLOB).unwrap();
        let search = matches.value_of(cli::ARG_SEARCH).unwrap();
        let dot = matches.value_of(cli::ARG_DOT);
        let json = matches.value_of(cli::ARG_JSON);
        dependency_graph(glob, search, dot, json);
    }

    if let Some(matches) = matches.subcommand_matches(cli::CMD_POST_MIGRATION_CLEANUP) {
        let glob_pattern = matches.value_of(cli::ARG_GLOB).unwrap();
        let glob_matcher = globset::Glob::new(glob_pattern).unwrap().compile_matcher();
        let search_path = matches.value_of(cli::ARG_SEARCH).unwrap();
        let search_path = std::fs::canonicalize(search_path).unwrap();
        post_migration_cleanup(search_path.as_path(), &glob_matcher);
    }

    if let Some(matches) = matches.subcommand_matches(cli::CMD_LIST_PROJECTS) {
        let glob_pattern = matches.value_of(cli::ARG_GLOB).unwrap();
        let glob_matcher = globset::Glob::new(glob_pattern).unwrap().compile_matcher();
        let search_path = matches.value_of(cli::ARG_SEARCH).unwrap();
        let search_path = std::fs::canonicalize(search_path).unwrap();
        list_projects(search_path.as_path(), &glob_matcher);
    }
}

fn find_files<'a>(
    search_path: &Path,
    glob_matcher: &'a globset::GlobMatcher,
) -> impl Iterator<Item = PathBuf> + 'a {
    let walk_builder = ignore::WalkBuilder::new(search_path);
    let cwd = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    walk_builder
        .build()
        .filter_map(move |result| -> Option<PathBuf> {
            let entry = result.unwrap();
            let meta = entry.metadata().unwrap();
            let path = std::fs::canonicalize(entry.path()).unwrap();
            let rel_path = path_extensions::relative_path(cwd.as_path(), path.as_path());
            if meta.is_file() && glob_matcher.is_match(rel_path) {
                Some(path)
            } else {
                None
            }
        })
}

fn parse_projects(
    search_path: &Path,
    glob_matcher: &globset::GlobMatcher,
) -> HashMap<PathBuf, Result<Project, Error>> {
    let meta = std::fs::metadata(search_path).unwrap();
    let mut todo: Vec<PathBuf> = if meta.is_file() {
        vec![search_path.to_path_buf()]
    } else {
        find_files(search_path, glob_matcher).collect()
    };

    let mut projects: HashMap<PathBuf, Option<Result<Project, Error>>> = todo
        .iter()
        .map(|project_path| (project_path.clone(), None))
        .collect();

    let mut new_todo = vec![];

    while !todo.is_empty() {
        for project_path in todo.drain(..) {
            let result = read_and_parse_project(project_path.clone());

            if let Ok(project) = &result {
                for project_path in project.dependencies.iter() {
                    if !projects.contains_key(project_path) {
                        projects.insert(project_path.clone(), None);
                        new_todo.push(project_path.clone());
                    }
                }
            }

            assert!(projects
                .get_mut(&project_path)
                .unwrap()
                .replace(result)
                .is_none());
        }

        todo.clear();
        std::mem::swap(&mut todo, &mut new_todo);
    }

    projects.into_iter().map(|(k, v)| (k, v.unwrap())).collect()
}

fn list_projects(search_path: &Path, glob_matcher: &globset::GlobMatcher) {
    let projects = parse_projects(search_path, glob_matcher);

    let cwd = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    let mut project_paths = projects
        .keys()
        .map(|project_path| path_extensions::relative_path(&cwd, project_path.as_path()))
        .collect::<Vec<_>>();
    project_paths.sort();
    for project_path in project_paths {
        println!("{}", project_path.display())
    }
}

fn post_migration_cleanup(search_path: &Path, glob_matcher: &globset::GlobMatcher) {
    // TODO(mickvangelderen): This is inefficient, we're parsing the projects twice.
    let projects = parse_projects(search_path, glob_matcher);

    for project_path in projects.keys() {
        let project_dir = project_path
            .parent()
            .expect("Failed to compute project directory path!");

        // let rel_project_path = path_extensions::relative_path(search_path, project_path.as_path());

        let mut reader = std::io::BufReader::new(std::fs::File::open(&project_path).unwrap());

        // Get rid of UTF-8 BOM if present.
        let bytes = std::io::BufRead::fill_buf(&mut reader).unwrap();
        let mut consume_count = 0;
        if &bytes[0..2] == "\u{FEFF}".as_bytes() {
            consume_count = 2;
        };
        // What the hell http://www.herongyang.com/Unicode/Notepad-Byte-Order-Mark-BOM-FEFF-EFBBBF.html
        if &bytes[0..3] == [0xEF, 0xBB, 0xBF] {
            consume_count = 3;
        };
        std::io::BufRead::consume(&mut reader, consume_count);

        let mut root = xmltree::Element::parse(&mut reader).unwrap();

        drop(reader);

        fn should_delete_element(element: &xmltree::Element) -> bool {
            match element.name.as_str() {
                "GenerateAssemblyInfo"
                | "Product"
                | "AssemblyTitle"
                | "Description"
                | "ProductVersion"
                | "Copyright"
                | "Company"
                | "NoWarn"
                | "TreatWarningsAsErrors"
                | "WarningsAsErrors"
                | "WarningLevel"
                | "DebugSymbols"
                | "DebugType"
                | "Optimize"
                | "OutputPath"
                | "DefineConstants"
                | "CodeAnalysisIgnoreBuiltInRuleSets"
                | "CodeAnalysisIgnoreBuiltInRules"
                | "CodeAnalysisFailOnMissingRules"
                | "AutoGenerateBindingRedirects"
                | "CodeAnalysisRuleSet"
                | "DefineDebug"
                | "DefineTrace"
                | "ErrorReport" => true,
                "PlatformTarget" => {
                    if let Some(v) = element.get_text() {
                        v.to_lowercase() == "anycpu"
                    } else {
                        false
                    }
                }
                "Compile" => {
                    if let Some(v) = element.attributes.get("Include") {
                        v.ends_with("SolutionInfo.cs")
                    } else {
                        false
                    }
                }
                "Import" => {
                    if let Some(v) = element.attributes.get("Project") {
                        v.ends_with("Microsoft.CSharp.Targets")
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }

        fn should_delete_element_pass_2(element: &xmltree::Element) -> bool {
            match element.name.as_str() {
                "PropertyGroup" | "ItemGroup" => element.children.iter().all(|node| match node {
                    xmltree::XMLNode::Text(text) => text.chars().all(|c| c.is_whitespace()),
                    _ => false,
                }),
                _ => false,
            }
        }

        fn omit_silly_elements<F>(element: &mut xmltree::Element, should_delete: F)
        where
            F: Fn(&xmltree::Element) -> bool + Copy,
        {
            element.children.retain(|node| match node {
                xmltree::XMLNode::Element(element) => !should_delete(element),
                _ => true,
            });

            element.children.iter_mut().for_each(|node| match node {
                xmltree::XMLNode::Element(element) => omit_silly_elements(element, should_delete),
                _ => {}
            });
        }

        omit_silly_elements(&mut root, should_delete_element);
        omit_silly_elements(&mut root, should_delete_element_pass_2);

        let mut writer =
            std::io::BufWriter::new(tempfile::NamedTempFile::new_in(project_dir).unwrap());

        root.write_with_config(
            &mut writer,
            xmltree::EmitterConfig {
                perform_escaping: false,
                perform_indent: true,
                write_document_declaration: false,
                ..Default::default()
            },
        )
        .unwrap();

        writer.into_inner().unwrap().persist(&project_path).unwrap();

        // fn count_project_guids(node: &xmltree::XMLNode) -> usize {
        //     match node {
        //         xmltree::XMLNode::Element(element) => {
        //             (if element.name == "ProjectGuid" {
        //                 1
        //             } else {
        //                 0
        //             }) + element.children.iter().map(count_project_guids).sum::<usize>()
        //         },
        //         _ => 0
        //     }
        // }
        // let project_guid_count = nodes.iter().map(count_project_guids).sum::<usize>();
        // println!("{}: project_guid_count: {}", rel_project_path.display(), project_guid_count);
    }
}

fn dependency_graph(glob: &str, search: &str, dot: Option<&str>, json: Option<&str>) {
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
                for path in project.dependencies.iter().cloned() {
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
                for dependency_path in project.dependencies.iter_mut() {
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

fn search_for_projects(glob_pattern: &str) -> HashMap<PathBuf, Option<Result<Project, Error>>> {
    glob::glob(glob_pattern)
        .unwrap()
        .filter_map(|project_path| {
            let project_path = project_path.unwrap();
            let project_path = std::fs::canonicalize(project_path).unwrap();
            let meta = std::fs::metadata(&project_path).unwrap();
            if !meta.is_file() {
                return None;
            }

            Some((project_path, None))
        })
        .collect()
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRoot {
    projects: Vec<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    path: PathBuf,
    is_sdk: bool,
    is_exe: bool,
    dependencies: Vec<PathBuf>,
}

#[derive(Debug)]
enum Error {
    Parse(roxmltree::Error),
    XmlTreeError(xmltree::Error),
    Io(std::io::Error),
}

impl From<roxmltree::Error> for Error {
    fn from(err: roxmltree::Error) -> Self {
        Self::Parse(err)
    }
}

impl From<xmltree::Error> for Error {
    fn from(err: xmltree::Error) -> Self {
        Self::XmlTreeError(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "failed to read project: {}", e),
            Error::Parse(e) => write!(f, "failed to parse project: {}", e),
            Error::XmlTreeError(e) => write!(f, "failed to parse project: {}", e),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::Io(ref e) => Some(e),
            Error::Parse(ref e) => Some(e),
            Error::XmlTreeError(ref e) => Some(e),
        }
    }
}

fn read_and_parse_project(project_path: PathBuf) -> Result<Project, Error> {
    let contents = std::fs::read_to_string(&project_path)?;

    let document = roxmltree::Document::parse(&contents)?;

    let project_dir = project_path
        .parent()
        .expect("Failed to compute project directory path!");

    let is_sdk = document
        .root()
        .children()
        .find(|node| node.tag_name().name() == "Project")
        .unwrap()
        .attribute("Sdk")
        .is_some();

    let is_exe = document
        .descendants()
        .find(|node| node.tag_name().name() == "OutputType")
        .map_or(false, |node| {
            matches!(node.text(), Some("Exe") | Some("WinExe"))
        });

    let dependencies = document
        .descendants()
        .filter_map(|node| -> Option<std::io::Result<PathBuf>> {
            if node.tag_name().name() != "ProjectReference" {
                return None;
            }
            let ref_path = PathBuf::from(node.attribute("Include")?);
            let ref_path = project_dir.join(&ref_path).simplify();
            Some(std::fs::canonicalize(ref_path))
        })
        .collect::<Result<Vec<PathBuf>, std::io::Error>>()?;

    Ok(Project {
        path: project_path,
        is_sdk,
        is_exe,
        dependencies,
    })
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
                    .dependencies
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
