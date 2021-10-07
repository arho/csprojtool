use crate::sln::types::*;
use siphasher::sip128::Hasher128;
use std::collections::BTreeMap;
use std::hash::Hash;
use std::io::Write;
use uuid::Uuid;

const HEADER: &'static str = r###"
Microsoft Visual Studio Solution File, Format Version 12.00
# Visual Studio Version 16
VisualStudioVersion = 16.0.30114.105
MinimumVisualStudioVersion = 10.0.40219.1
"###;

const FOLDER_UUID: Uuid = Uuid::from_bytes(0x2150E3338FDC42A394741A3956D46DE8u128.to_be_bytes());
const PROJECT_UUID: Uuid = Uuid::from_bytes(0xFAE04EC0301F11D3BF4B00C04F79EFBCu128.to_be_bytes());

#[derive(Debug, Clone)]
pub enum Node {
    Project(Project),
    Directory(Directory),
}

#[derive(Debug, Clone)]
pub struct SolutionFile {
    pub root: InnerRootDirectory,
}

impl SolutionFile {
    pub fn new(root: Directory) -> Self {
        Self {
            root: InnerRootDirectory::new(root),
        }
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(HEADER.as_bytes())?;

        self.root.write_projects(writer)?;

        write!(writer, "Global\n")?;
        self.write_global_section_solution_configuration_platforms(writer)?;
        self.write_global_section_solution_properties(writer)?;
        self.root.write_project_configurations(writer)?;
        self.root.write_nested_projects(writer)?;
        write!(writer, "EndGlobal\n")?;

        Ok(())
    }

    fn write_global_section_solution_configuration_platforms<W: Write>(
        &self,
        writer: &mut W,
    ) -> std::io::Result<()> {
        write!(
            writer,
            "\tGlobalSection(SolutionConfigurationPlatforms) = preSolution\n"
        )?;

        for conf in CONFIGURATIONS {
            for arch in PROCESSOR_ARCHITECTURES {
                write!(writer, "\t\t{0}|{1} = {0}|{1}\n", conf, arch)?;
            }
        }

        write!(writer, "\tEndGlobalSection\n")?;

        Ok(())
    }

    fn write_global_section_solution_properties<W: Write>(
        &self,
        writer: &mut W,
    ) -> std::io::Result<()> {
        write!(
            writer,
            "\tGlobalSection(SolutionProperties) = preSolution\n"
        )?;
        write!(writer, "\t\tHideSolutionNode = FALSE\n")?;
        write!(writer, "\tEndGlobalSection\n")?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub guid: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct Directory {
    pub nodes: BTreeMap<String, Node>,
}

#[derive(Debug, Clone)]
pub enum InnerNode {
    Directory(InnerDirectory),
    Project(InnerProject),
}

impl InnerNode {
    pub fn new(path: &str, name: String, node: Node) -> Self {
        match node {
            Node::Directory(dir) => Self::Directory(InnerDirectory::new(path, name, dir)),
            Node::Project(proj) => Self::Project(InnerProject::new(path, name, proj)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InnerRootDirectory {
    pub nodes: Vec<InnerNode>,
}

impl InnerRootDirectory {
    pub fn new(root: Directory) -> Self {
        let nodes = root
            .nodes
            .into_iter()
            .map(|(name, node)| InnerNode::new("", name, node))
            .collect();

        Self { nodes }
    }

    fn write_projects<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for node in self.nodes.iter() {
            match node {
                InnerNode::Directory(dir) => dir.write_projects(writer)?,
                InnerNode::Project(proj) => proj.write_project(writer)?,
            }
        }

        Ok(())
    }

    fn write_project_configurations<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write!(
            writer,
            "\tGlobalSection(ProjectConfigurationPlatforms) = postSolution\n"
        )?;

        for node in self.nodes.iter() {
            match node {
                InnerNode::Directory(dir) => dir.write_project_configurations(writer)?,
                InnerNode::Project(proj) => proj.write_project_configuration(writer)?,
            }
        }

        write!(writer, "\tEndGlobalSection\n")?;
        Ok(())
    }

    fn write_nested_projects<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write!(writer, "\tGlobalSection(NestedProjects) = preSolution\n")?;

        for node in self.nodes.iter() {
            match node {
                InnerNode::Directory(dir) => dir.write_nested_projects(writer)?,
                InnerNode::Project(_) => {
                    // Projects are implicitly placed under the root if the nested project is omitted.
                }
            }
        }

        write!(writer, "\tEndGlobalSection\n")?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InnerDirectory {
    pub path: String,
    pub guid: Uuid,
    pub name: String,
    pub nodes: Vec<InnerNode>,
}

impl InnerDirectory {
    pub fn new(path: &str, name: String, dir: Directory) -> Self {
        let path = join_str_path(path, &name);
        let guid = guid_from_hash(&path);
        let nodes = dir
            .nodes
            .into_iter()
            .map(|(name, node)| InnerNode::new(&path, name, node))
            .collect();
        Self {
            path,
            guid,
            name,
            nodes,
        }
    }
}

impl InnerDirectory {
    fn contains_single_project(&self) -> Option<&InnerProject> {
        if self.nodes.len() != 1 {
            return None;
        }
        match self.nodes.first().unwrap() {
            InnerNode::Project(proj) => Some(proj),
            _ => None,
        }
    }

    fn write_projects<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self.contains_single_project() {
            Some(proj) if proj.name == self.name => {
                // Skip writing this directory.
            }
            _ => {
                write!(
                    writer,
                    "Project(\"{{{0:X}}}\") = \"{1}\", \"{1}\", \"{{{2:X}}}\"\nEndProject\n",
                    FOLDER_UUID, &self.name, self.guid
                )?;
            }
        }

        for node in self.nodes.iter() {
            match node {
                InnerNode::Directory(dir) => dir.write_projects(writer)?,
                InnerNode::Project(proj) => proj.write_project(writer)?,
            }
        }

        Ok(())
    }

    fn write_project_configurations<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for node in self.nodes.iter() {
            match node {
                InnerNode::Directory(dir) => dir.write_project_configurations(writer)?,
                InnerNode::Project(proj) => proj.write_project_configuration(writer)?,
            }
        }

        Ok(())
    }

    fn write_nested_projects<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        fn write_nested_project<W: Write>(
            writer: &mut W,
            parent: Uuid,
            child: Uuid,
        ) -> std::io::Result<()> {
            write!(writer, "\t\t{{{0:X}}} = {{{1:X}}}\n", child, parent)
        }

        for node in self.nodes.iter() {
            match node {
                InnerNode::Directory(dir) => {
                    match dir.contains_single_project() {
                        Some(proj) if proj.name == dir.name => {
                            // This directory is skipped so we will write a link from the parent dir to the only project directly here.
                            write_nested_project(writer, self.guid, proj.guid)?;
                        }
                        _ => {
                            write_nested_project(writer, self.guid, dir.guid)?;
                            dir.write_nested_projects(writer)?;
                        }
                    }
                }
                InnerNode::Project(proj) => write_nested_project(writer, self.guid, proj.guid)?,
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct InnerProject {
    pub path: String,
    pub guid: Uuid,
    pub name: String,
}

impl InnerProject {
    pub fn new(path: &str, name: String, proj: Project) -> Self {
        let path = join_str_path(path, &name);
        let name = name.strip_suffix(".csproj").unwrap().to_owned();
        let guid = proj.guid.unwrap_or_else(|| guid_from_hash(&path));
        Self { path, name, guid }
    }

    fn write_project<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write!(
            writer,
            "Project(\"{{{0:X}}}\") = \"{1}\", \"{2}\", \"{{{3:X}}}\"\nEndProject\n",
            PROJECT_UUID, self.name, self.path, self.guid
        )
    }

    pub fn write_project_configuration<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        for conf in CONFIGURATIONS {
            for arch in PROCESSOR_ARCHITECTURES {
                for suffix in ["ActiveCfg", "Build.0"] {
                    write!(
                        writer,
                        "\t\t{{{guid:X}}}.{conf}|{arch}.{suffix} = {conf}|Any CPU\n",
                        guid = self.guid,
                        conf = conf,
                        arch = arch,
                        suffix = suffix,
                    )?;
                }
            }
        }

        Ok(())
    }
}

pub fn guid_from_hash<H: Hash>(value: H) -> Uuid {
    let mut hasher = siphasher::sip128::SipHasher::new();
    value.hash(&mut hasher);
    Uuid::from_bytes(hasher.finish128().as_bytes())
}

pub fn join_str_path(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.to_owned()
    } else {
        [a, "\\", b].iter().copied().collect()
    }
}
