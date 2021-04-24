use crate::*;

pub fn post_migration_cleanup(search_path: &Path, glob_matcher: &globset::GlobMatcher) {
    // TODO(mickvangelderen): This is inefficient, we're parsing the projects twice.
    let projects = parse_projects(search_path, glob_matcher);

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
        if let Err(e) = post_migration_cleanup_one(project_path.as_path()) {
            panic!("Failed to migrate {}: {}", project_path.display(), e)
        }
    }
}

fn post_migration_cleanup_one(project_path: &Path) -> Result<(), Error> {
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

    let mut root = xmltree::Element::parse(&mut reader)?;

    drop(reader);

    process_tree(&mut root, process_element);

    let mut writer = std::io::BufWriter::new(tempfile::NamedTempFile::new_in(project_dir)?);

    root.write_with_config(
        &mut writer,
        xmltree::EmitterConfig {
            perform_escaping: true,
            perform_indent: true,
            write_document_declaration: false,
            ..Default::default()
        },
    )
    .unwrap();

    writer.into_inner().unwrap().persist(&project_path)?;

    Ok(())
}

fn process_element(element: &mut xmltree::Element) {
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
                    | "AutoGenerateBindingRedirects"
                    | "CodeAnalysisRuleSet"
                    | "DefineDebug"
                    | "DefineTrace"
                    | "DocumentationFile" // sdk projects support <GenerateDocumentationFile>true</GenerateDocumentationFile> which can be enabled for all projects through Directory.Build.props
                    | "ErrorReport" => {}
                    "StartupObject" => {
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
