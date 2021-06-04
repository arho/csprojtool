use crate::csproj::*;
use crate::*;

pub struct PostMigrationCleanupOptions {
    pub search_path: PathBuf,
    pub glob_matcher: globset::GlobMatcher,
    pub follow_project_references: bool,
    pub clean_app_configs: bool,
}

pub fn post_migration_cleanup(options: &PostMigrationCleanupOptions) {
    let PostMigrationCleanupOptions {
        ref search_path,
        ref glob_matcher,
        follow_project_references,
        clean_app_configs,
    } = *options;

    // TODO(mickvangelderen): This is inefficient, we're parsing the projects twice.
    let projects = parse_projects(search_path, glob_matcher, follow_project_references);

    let cwd = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();

    for project_path in projects.into_iter().filter_map(|(path, project)| {
        let rel_path = path_extensions::relative_path(cwd.as_path(), path.as_path());
        match project {
            Ok(project) => {
                if project.is_sdk {
                    println!("Migrating sdk project {}", rel_path.display());
                    Some(path)
                } else {
                    println!("Skipping non-sdk project {}", rel_path.display());
                    None
                }
            }
            Err(err) => {
                eprintln!("Failed to parse {}: {}", rel_path.display(), err);
                None
            }
        }
    }) {
        if clean_app_configs {
            let project_dir = project_path.parent().unwrap();
            let cwd = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
            for app_config_path in find_app_configs(project_dir).unwrap() {
                let app_config_path = app_config_path.unwrap();
                let rel_path =
                    path_extensions::relative_path(cwd.as_path(), app_config_path.as_path());
                println!("Cleaning up app config {}", rel_path.display());
                if let Err(e) = cleanup_app_config(app_config_path.as_path()) {
                    panic!(
                        "Failed to clean up app config {}: {}",
                        app_config_path.display(),
                        e
                    );
                }
            }
        }

        if let Err(e) = cleanup_csproj(project_path.as_path()) {
            panic!("Failed to migrate {}: {}", project_path.display(), e)
        }
    }
}

fn find_app_configs(
    project_dir: &Path,
) -> Result<impl Iterator<Item = Result<PathBuf, Error>>, Error> {
    let glob_matcher = globset::GlobBuilder::new("**/app.config")
        .case_insensitive(true)
        .build()
        .unwrap()
        .compile_matcher();

    Ok(std::fs::read_dir(project_dir)?.into_iter().filter_map(
        move |entry| -> Option<Result<PathBuf, Error>> {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Some(Err(e.into())),
            };

            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(e) => return Some(Err(e.into())),
            };

            let path = entry.path();

            if meta.is_file() && glob_matcher.is_match(path.as_path()) {
                Some(Ok(path))
            } else {
                None
            }
        },
    ))
}

fn cleanup_app_config(path: &Path) -> Result<(), Error> {
    let project_dir = path.parent().unwrap();

    let mut root = read_xml_file(path)?;

    process_tree(&mut root, app_config_element_transform);

    let mut writer = std::io::BufWriter::new(tempfile::NamedTempFile::new_in(project_dir)?);

    root.write_with_config(
        &mut writer,
        xmltree::EmitterConfig {
            perform_escaping: true,
            perform_indent: true,
            write_document_declaration: true,
            line_separator: "\r\n".into(),
            ..Default::default()
        },
    )
    .unwrap();

    if all_children_whitespace(&root) {
        std::fs::remove_file(path)?;
    } else {
        writer.into_inner().unwrap().persist(&path)?;
    }

    Ok(())
}

fn app_config_element_transform(element: &mut xmltree::Element) {
    let mut new_children = Vec::with_capacity(element.children.len());

    for old_child in element.children.drain(..) {
        match old_child {
            xmltree::XMLNode::Element(old_child) => {
                match old_child.name.as_str() {
                    "assemblyBinding" // these should be auto-generated
                    | "supportedRuntime" => {} // supportedRuntime is messy, just target the right framework. See https://stackoverflow.com/a/21578128/4127458
                    _ => new_children.push(xmltree::XMLNode::Element(old_child)),
                }
            }
            other => new_children.push(other),
        }
    }

    // Omit group elements without children
    element.children = new_children
        .into_iter()
        .filter_map(|new_child| match new_child {
            xmltree::XMLNode::Element(element) => match element.name.as_str() {
                "runtime" | "startup" => {
                    if all_children_whitespace(&element) {
                        None
                    } else {
                        Some(xmltree::XMLNode::Element(element))
                    }
                }
                _ => Some(xmltree::XMLNode::Element(element)),
            },
            other => Some(other),
        })
        .collect();
}

fn cleanup_csproj(project_path: &Path) -> Result<(), Error> {
    let project_dir = project_path
        .parent()
        .expect("Failed to compute project directory path!");

    let mut root = read_xml_file(project_path)?;

    process_tree(&mut root, csproj_element_transform);

    let mut writer = std::io::BufWriter::new(tempfile::NamedTempFile::new_in(project_dir)?);

    root.write_with_config(
        &mut writer,
        xmltree::EmitterConfig {
            perform_escaping: true,
            perform_indent: true,
            write_document_declaration: false,
            line_separator: "\r\n".into(),
            ..Default::default()
        },
    )
    .unwrap();

    writer.into_inner().unwrap().persist(&project_path)?;

    Ok(())
}

fn csproj_element_transform(element: &mut xmltree::Element) {
    let mut new_children = Vec::with_capacity(element.children.len());

    for old_child in element.children.drain(..) {
        match old_child {
            xmltree::XMLNode::Element(mut old_child) => {
                match old_child.name.as_str() {
                    "PropertyGroup" => {
                        // Merge the children of PropertyGroups with the same attributes.
                        if let Some(new_child) = new_children
                            .iter_mut()
                            .filter_map(|new_child| {
                                if let xmltree::XMLNode::Element(new_child) = new_child {
                                    if new_child.name == "PropertyGroup"
                                        && new_child.attributes == old_child.attributes
                                    {
                                        return Some(new_child);
                                    }
                                }
                                None
                            })
                            .next()
                        {
                            new_child.children.extend(old_child.children.drain(..));
                        } else {
                            new_children.push(xmltree::XMLNode::Element(old_child));
                        }
                    }
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
                    | "CodeAnalysisRuleSet"
                    | "DefineDebug"
                    | "DefineTrace"
                    | "DocumentationFile" // sdk projects support <GenerateDocumentationFile>true</GenerateDocumentationFile> which can be enabled for all projects through Directory.Build.props
                    | "ErrorReport" => {}
                    "StartupObject" | "PostBuildEvent" => {
                        if !old_child.children.is_empty() {
                            new_children.push(xmltree::XMLNode::Element(old_child))
                        }
                    }
                    "PlatformTarget" => {
                        if let Some(v) = old_child.get_text() {
                            if v.to_lowercase() != "anycpu" {
                                new_children.push(xmltree::XMLNode::Element(old_child))
                            }
                        }
                    }
                    "Compile" => {
                        if let Some(v) = old_child.attributes.get("Include") {
                            if !v.ends_with("SolutionInfo.cs") {
                                new_children.push(xmltree::XMLNode::Element(old_child))
                            }
                        } else {
                            new_children.push(xmltree::XMLNode::Element(old_child))
                        }
                    }
                    "Import" => {
                        if let Some(v) = old_child.attributes.get("Project") {
                            if !v.ends_with("Microsoft.CSharp.Targets") {
                                new_children.push(xmltree::XMLNode::Element(old_child))
                            }
                        } else {
                            new_children.push(xmltree::XMLNode::Element(old_child))
                        }
                    }
                    _ => new_children.push(xmltree::XMLNode::Element(old_child)),
                }
            }
            other => new_children.push(other),
        }
    }

    // Omit group elements without children
    element.children = new_children
        .into_iter()
        .filter_map(|new_child| match new_child {
            xmltree::XMLNode::Element(element) => match element.name.as_str() {
                "PropertyGroup" | "ItemGroup" => {
                    if all_children_whitespace(&element) {
                        None
                    } else {
                        Some(xmltree::XMLNode::Element(element))
                    }
                }
                _ => Some(xmltree::XMLNode::Element(element)),
            },
            other => Some(other),
        })
        .collect();
}

fn process_tree<F>(element: &mut xmltree::Element, process_element: F)
where
    F: Fn(&mut xmltree::Element) + Copy,
{
    for node in element.children.iter_mut() {
        match node {
            xmltree::XMLNode::Element(element) => process_tree(element, process_element),
            _ => {}
        }
    }
    process_element(element)
}

fn all_children_whitespace(element: &xmltree::Element) -> bool {
    element.children.iter().all(|node| match node {
        xmltree::XMLNode::Text(text) => text.chars().all(|c| c.is_whitespace()),
        _ => false,
    })
}
