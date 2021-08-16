extern crate regex;
use regex::Regex;
use crate::csproj::*;
use crate::path_extensions::*;
use siphasher::sip128::Hasher128;
use std::collections::BTreeMap;
use std::collections::HashMap; 
use std::hash::Hash;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::{io::Write, path::PathBuf, path::Path};
use uuid::Uuid;

const HEADER: &'static str = r###"
Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 16
VisualStudioVersion = 16.0.30114.105
MinimumVisualStudioVersion = 10.0.40219.1
"###;

const FOLDER_UUID_STR: &'static str = "2150E333-8FDC-42A3-9474-1A3956D46DE8";
const FOLDER_UUID: Uuid = Uuid::from_bytes(0x2150E3338FDC42A394741A3956D46DE8u128.to_be_bytes());

const PROJECT_UUID_STR: &'static str = "FAE04EC0-301F-11D3-BF4B-00C04F79EFBC";
const PROJECT_UUID: Uuid = Uuid::from_bytes(0xFAE04EC0301F11D3BF4B00C04F79EFBCu128.to_be_bytes());

#[derive(Debug)]
pub struct SolutionFile {
    projects: Vec<Project>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Project {
    path: PathBuf,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct Directory {
    directories: BTreeMap<String, Directory>,
    projects: BTreeMap<String, Project>,
}

fn compute_hierarchy(projects: &[Project]) -> Directory {
    let mut root = Directory::default();

    for project in projects.iter().cloned() {
        let mut dirnames: Vec<String> = project
            .path
            .components()
            .map(|comp| match comp {
                std::path::Component::Normal(val) => val.to_str().unwrap().to_owned(),
                _ => panic!("Expected a relative path"),
            })
            .collect();

        if let Some(filename) = dirnames.pop() {
            let mut dir = &mut root;

            for dirname in dirnames {
                dir = dir
                    .directories
                    .entry(dirname)
                    .or_insert(Directory::default());
            }

            dir.projects.insert(filename, project);
        }
    }

    root
}

pub fn read_and_parse_solutions(
    search_path: &Path,
    glob_matcher: &globset::GlobMatcher,
) -> HashMap<PathBuf, Result<SolutionFile, Error>> {
    let meta = std::fs::metadata(search_path).unwrap();
    let todo: Vec<PathBuf> = if meta.is_file() {
        vec![search_path.to_path_buf()]
    } else {
        find_files(search_path, glob_matcher).collect()
    };

    todo.iter()
        .map(|sln_path| (sln_path.clone(), read_and_parse_solution(sln_path.clone())))
        .collect()
}

pub fn read_and_parse_solution(solution_path: PathBuf) -> Result<SolutionFile, Error> {
    
    // cargo-cult programming here, needs error handling improvement...

    let mut projects = Vec::<Project>::new();
    let file = File::open(&solution_path).unwrap();
    let reader = BufReader::new(file);
    let rg_proj = Regex::new(r###"Project\("(?P<type_id>\{.*\})"\) = "(?P<name>.*)", "(?P<path>.*)", "(?P<proj_id>\{.*\})""###).unwrap();
    let sln_dir = solution_path
        .parent()
        .expect("Failed to compute solution directory path!");
    for (_index, line) in reader.lines().enumerate() {
        let line = line.unwrap(); // Ignore errors.        
        match rg_proj.captures(&line) {            
            Some(_match) => {
                let path = _match.name("path").unwrap().as_str();
                let proj_path = PathBuf::from(path);
                let proj_path = sln_dir.join(&proj_path).simplify();
                //let proj_path = std::fs::canonicalize(proj_path).unwrap();          
                let proj = Project{
                    path: proj_path
                };
                projects.push(proj);
            },
            None    => {}
        }
    }  
   
    Ok(SolutionFile {
        projects: projects,
    })
}


impl SolutionFile {
    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(HEADER.as_bytes())?;

        let root = compute_hierarchy(&self.projects);

        root.write_projects(writer)?;

        Ok(())
    }

    pub fn projects(&self) -> Vec<Project> {
        self.projects.clone()
    }

}
impl Project {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

impl Directory {
    fn write_projects<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.write_projects_inner(writer, "".into())
    }

    fn write_projects_inner<W: Write>(
        &self,
        writer: &mut W,
        prefix: String,
    ) -> std::io::Result<()> {
        for (dirname, dir) in self.directories.iter() {
            let dirpath = [prefix.as_str(), "/", dirname.as_str()]
                .iter()
                .copied()
                .collect::<String>();
            let guid = guid_from_hash(&dirpath);

            write!(writer, "Project(\"{{2150E333-8FDC-42A3-9474-1A3956D46DE8}}\") = \"{0}\", \"{0}\", \"{{{1:X}}}\"\nEndProject\n", dirname, guid)?;

            dir.write_projects_inner(writer, dirpath)?;
        }

        for (projname, proj) in self.projects.iter() {
            let guid = guid_from_hash(&proj.path);
            write!(writer, "Project(\"{{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}}\") = \"{0}\", \"{1}\", \"{{{2:X}}}\"\nEndProject\n", projname, &proj.path.display(), guid)?;
        }

        Ok(())
    }
}

fn guid_from_hash<H: Hash>(value: H) -> Uuid {
    let mut hasher = siphasher::sip128::SipHasher::new();
    value.hash(&mut hasher);
    Uuid::from_bytes(hasher.finish128().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_correct() {
        assert_eq!(
            FOLDER_UUID
                .to_hyphenated()
                .encode_upper(&mut Uuid::encode_buffer()),
            FOLDER_UUID_STR
        );
        assert_eq!(
            PROJECT_UUID
                .to_hyphenated()
                .encode_upper(&mut Uuid::encode_buffer()),
            PROJECT_UUID_STR
        );
    }

    #[test]
    fn compute_hierarchy_works() {
        let projects = [
            Project {
                path: PathBuf::from("Common/Organization.Geometry/Organization.Geometry.csproj")
            },
            Project {
                path: PathBuf::from("Common/Organization.Metrics/Organization.Metrics.csproj")
            },
            Project {
                path: PathBuf::from("Application/Organization.HeatmapVisualizer/Organization.HeatmapVisualizer.csproj")
            }
        ];

        let actual = compute_hierarchy(&projects);

        let expected = Directory {
            directories: [
                ("Common".to_owned(), Directory {
                    directories: [
                        ("Organization.Geometry".to_owned(), Directory {
                            directories: Default::default(),
                            projects: [("Organization.Geometry.csproj".to_owned(), Project { 
                                path: PathBuf::from("Common/Organization.Geometry/Organization.Geometry.csproj")
                            })].iter().cloned().collect(),
                        }),
                        ("Organization.Metrics".to_owned(), Directory {
                            directories: Default::default(),
                            projects: [("Organization.Metrics.csproj".to_owned(), Project { 
                                path: PathBuf::from("Common/Organization.Metrics/Organization.Metrics.csproj")
                            })].iter().cloned().collect(),
                        }),
                    ].iter().cloned().collect(),
                    projects: Default::default(),
                }),
                ("Application".to_owned(), Directory {
                    directories: [
                        ("Organization.HeatmapVisualizer".to_owned(), Directory {
                            directories: Default::default(),
                            projects: [("Organization.HeatmapVisualizer.csproj".to_owned(), Project { 
                                path: PathBuf::from("Application/Organization.HeatmapVisualizer/Organization.HeatmapVisualizer.csproj")
                            })].iter().cloned().collect(),
                        }),
                    ].iter().cloned().collect(),
                    projects: Default::default(),
                })
            ].iter().cloned().collect(),
            projects: Default::default(),
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn writing_works() {
        let sln_file = SolutionFile {
            projects: vec![
                Project {
                    path: PathBuf::from("Common/Organization.Geometry/Organization.Geometry.csproj")
                },
                Project {
                    path: PathBuf::from("Common/Organization.Metrics/Organization.Metrics.csproj")
                },
                Project {
                    path: PathBuf::from("Application/Organization.HeatmapVisualizer/Organization.HeatmapVisualizer.csproj")
                }
            ],
        };
        
        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        sln_file.write(&mut cursor).unwrap();
        let result = String::from_utf8(cursor.into_inner()).unwrap();
        println!("{}", result);
        assert_eq!(&result, r###"
Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 16
VisualStudioVersion = 16.0.30114.105
MinimumVisualStudioVersion = 10.0.40219.1
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Application", "Application", "{D14B4492-4424-6894-BA35-396D0AF89F98}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Organization.HeatmapVisualizer", "Organization.HeatmapVisualizer", "{A1331AFC-7AE5-2B9B-59FA-3EE384ED7C3B}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "Organization.HeatmapVisualizer.csproj", "Application/Organization.HeatmapVisualizer/Organization.HeatmapVisualizer.csproj", "{8633FE71-9780-C7AE-5AC8-420CA07BD95A}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Common", "Common", "{24F9A5B7-70DF-8251-85F7-5AFA2AB69D1B}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Organization.Geometry", "Organization.Geometry", "{7A57460C-0115-0256-E323-6D99663C2620}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "Organization.Geometry.csproj", "Common/Organization.Geometry/Organization.Geometry.csproj", "{5290E9A3-4EBD-CCAE-06E3-EB3209B673C1}"
EndProject
Project("{2150E333-8FDC-42A3-9474-1A3956D46DE8}") = "Organization.Metrics", "Organization.Metrics", "{DFF66A48-EA4B-8F13-B422-AF1E227639FE}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "Organization.Metrics.csproj", "Common/Organization.Metrics/Organization.Metrics.csproj", "{72D03072-22F4-AF96-0CE3-FBD115166B07}"
EndProject
"###);
    }
}
