#[allow(dead_code)]
pub fn assert_svg_element(svg: &str, tag: &str, attributes: &[(&str, &str)]) {
    let document = roxmltree::Document::parse(svg).unwrap();
    assert!(
        document.descendants().any(|node| node.has_tag_name(tag)
            && attributes
                .iter()
                .all(|(name, value)| node.attribute(*name) == Some(*value))),
        "missing {tag} with {attributes:?} in {svg}"
    );
}

#[allow(dead_code)]
pub fn svg_path_commands(svg: &str, attribute: &str, value: &str) -> Vec<svgtypes::PathSegment> {
    let document = roxmltree::Document::parse(svg).unwrap();
    let node = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute(attribute) == Some(value))
        .unwrap();
    svgtypes::PathParser::from(node.attribute("d").unwrap())
        .map(Result::unwrap)
        .collect()
}
