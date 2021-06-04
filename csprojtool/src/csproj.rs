use crate::path_extensions::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

pub fn search_for_projects(glob_pattern: &str) -> HashMap<PathBuf, Option<Result<Project, Error>>> {
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
pub struct JsonRoot {
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub path: PathBuf,
    pub is_sdk: bool,
    pub is_exe: bool,
    pub target_frameworks: Vec<String>,
    pub project_references: Vec<PathBuf>,
    pub package_references: Vec<PackageReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageReference {
    pub name: String,
    pub version: String,
}

#[derive(Debug)]
pub enum Error {
    Parse(roxmltree::Error),
    XmlTreeError(xmltree::Error),
    XmlTreeParseError(xmltree::ParseError),
    PersistError(tempfile::PersistError),
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

impl From<xmltree::ParseError> for Error {
    fn from(err: xmltree::ParseError) -> Self {
        Self::XmlTreeParseError(err)
    }
}

impl From<tempfile::PersistError> for Error {
    fn from(err: tempfile::PersistError) -> Self {
        Self::PersistError(err)
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
            Error::XmlTreeParseError(e) => write!(f, "failed to parse project: {}", e),
            Error::PersistError(e) => write!(f, "failed to parse project: {}", e),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            Error::Io(ref e) => Some(e),
            Error::Parse(ref e) => Some(e),
            Error::XmlTreeError(ref e) => Some(e),
            Error::XmlTreeParseError(ref e) => Some(e),
            Error::PersistError(ref e) => Some(e),
        }
    }
}

pub fn parse_projects(
    search_path: &Path,
    glob_matcher: &globset::GlobMatcher,
    follow_project_references: bool,
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
                if follow_project_references {
                    for project_path in project.project_references.iter() {
                        if !projects.contains_key(project_path) {
                            projects.insert(project_path.clone(), None);
                            new_todo.push(project_path.clone());
                        }
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
            let rel_path = relative_path(cwd.as_path(), path.as_path());
            if meta.is_file() && glob_matcher.is_match(rel_path) {
                Some(path)
            } else {
                None
            }
        })
}

pub fn read_and_parse_project(project_path: PathBuf) -> Result<Project, Error> {
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

    let target_frameworks = {
        let target_frameworks_iter = document
            .descendants()
            .filter_map(|node| {
                if node.tag_name().name() == "TargetFrameworks" {
                    node.text()
                } else {
                    None
                }
            })
            .flat_map(|text| text.split(';'))
            .map(str::to_owned);

        let target_framework_iter = document
            .descendants()
            .filter_map(|node| {
                if node.tag_name().name() == "TargetFramework" {
                    node.text()
                } else {
                    None
                }
            })
            .map(str::to_owned);

        // Old style
        let target_framework_version_iter = document.descendants().filter_map(|node| {
            if node.tag_name().name() == "TargetFrameworkVersion" {
                node.text()
                    .map(parse_target_framework_version)
                    .expect("Failed to parse framework version!")
            } else {
                None
            }
        });

        let mut target_frameworks = target_frameworks_iter
            .chain(target_framework_iter)
            .chain(target_framework_version_iter)
            .collect::<Vec<_>>();

        target_frameworks.sort();
        target_frameworks.dedup();
        target_frameworks
    };

    let project_references = document
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

    let package_references = document
        .descendants()
        .filter_map(|node| -> Option<PackageReference> {
            if node.tag_name().name() != "PackageReference" {
                return None;
            }
            Some(PackageReference {
                name: node.attribute("Include")?.to_string(),
                version: node.attribute("Version")?.to_string(),
            })
        })
        .collect::<Vec<_>>();
    Ok(Project {
        path: project_path,
        is_sdk,
        is_exe,
        target_frameworks,
        project_references,
        package_references,
    })
}

fn strip_bom<R: std::io::BufRead>(reader: &mut R) {
    // Get rid of UTF-8 BOM if present.
    let bytes = std::io::BufRead::fill_buf(reader).unwrap();

    let mut consume_count = 0;
    if &bytes[0..2] == "\u{FEFF}".as_bytes() {
        consume_count = 2;
    };

    // What the hell http://www.herongyang.com/Unicode/Notepad-Byte-Order-Mark-BOM-FEFF-EFBBBF.html
    if &bytes[0..3] == [0xEF, 0xBB, 0xBF] {
        consume_count = 3;
    };

    std::io::BufRead::consume(reader, consume_count);
}

pub fn read_xml_file<P: AsRef<Path>>(path: P) -> Result<xmltree::Element, Error> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path.as_ref())?);
    strip_bom(&mut reader);
    Ok(xmltree::Element::parse(&mut reader)?)
}

fn parse_target_framework_version(text: &str) -> Option<String> {
    lazy_static::lazy_static! {
        static ref RE: regex::Regex = regex::Regex::new(r"^\s*v(\d)\.(\d)(?:\.(\d))?\s*$").unwrap();
    }
    RE.captures(text).map(|c| {
        format!(
            "net{}{}{}",
            &c[1],
            &c[2],
            c.get(3).map(|m| m.as_str()).unwrap_or("")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::parse_target_framework_version;

    #[test]
    fn parse_target_framework_version_works() {
        assert_eq!(
            parse_target_framework_version(" v3.5 "),
            Some(String::from("net35"))
        );
        assert_eq!(
            parse_target_framework_version("v4.7.1"),
            Some(String::from("net471"))
        );
    }
}
