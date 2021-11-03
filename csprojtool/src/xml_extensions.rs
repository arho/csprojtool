use std::path::Path;

use xmltree::{Element, XMLNode};

use crate::csproj::Error;

pub fn process_tree<F>(element: &mut Element, process_element: F)
where
    F: FnMut(&mut Element),
{
    process_tree_inner(element, process_element);
}

fn process_tree_inner<F>(element: &mut Element, mut process_element: F) -> F
where
    F: FnMut(&mut Element),
{
    for element in child_elements_mut(element) {
        process_element = process_tree_inner(element, process_element);
    }

    process_element(element);

    process_element
}

pub fn depth_first_visit_nodes<F>(node: &mut XMLNode, visit: F)
where
    F: FnMut(&mut XMLNode),
{
    inner_depth_first_visit_nodes(node, visit);
}

fn inner_depth_first_visit_nodes<F>(node: &mut XMLNode, mut visit: F) -> F
where
    F: FnMut(&mut XMLNode),
{
    if let XMLNode::Element(element) = node {
        for child in element.children.iter_mut() {
            visit = inner_depth_first_visit_nodes(child, visit);
        }
    }

    visit(node);

    visit
}

pub fn all_children_whitespace(element: &Element) -> bool {
    element.children.iter().all(|node| match node {
        xmltree::XMLNode::Text(text) => text.chars().all(|c| c.is_whitespace()),
        _ => false,
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

pub fn read_xml_file<P: AsRef<Path>>(path: P) -> Result<Element, Error> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path.as_ref())?);
    strip_bom(&mut reader);
    Ok(Element::parse(&mut reader)?)
}

pub fn transform_xml_file<F>(file_path: &Path, transform: F) -> Result<(), Error>
where
    F: FnOnce(Element) -> Option<Element>,
{
    let dir_path = file_path.parent().unwrap();

    if let Some(root) = transform(read_xml_file(file_path)?) {
        let mut writer = std::io::BufWriter::new(tempfile::NamedTempFile::new_in(dir_path)?);

        let write_document_declaration = root.attributes.get("Sdk").is_none();

        root.write_with_config(
            &mut writer,
            xmltree::EmitterConfig {
                perform_escaping: true,
                perform_indent: true,
                write_document_declaration,
                line_separator: "\r\n".into(),
                ..Default::default()
            },
        )
        .unwrap();

        writer.into_inner().unwrap().persist(&file_path)?;
    }

    Ok(())
}

fn node_as_element(node: &XMLNode) -> Option<&Element> {
    match node {
        XMLNode::Element(element) => Some(element),
        _ => None,
    }
}

pub fn child_elements(element: &Element) -> impl Iterator<Item = &Element> {
    element.children.iter().filter_map(node_as_element)
}

fn node_as_element_mut(node: &mut XMLNode) -> Option<&mut Element> {
    match node {
        XMLNode::Element(element) => Some(element),
        _ => None,
    }
}

pub fn child_elements_mut(element: &mut Element) -> impl Iterator<Item = &mut Element> {
    element.children.iter_mut().filter_map(node_as_element_mut)
}
