use std::collections::HashMap;
use std::fmt::Write;

use crate::hierarchy::{build_hierarchy, HierarchyNode};
use crate::types::{ElementData, ExcalidrawElement};

pub fn generate_mermaid(elements: &[&ExcalidrawElement]) -> String {
    // Build index: id -> element
    let index: HashMap<&str, &ExcalidrawElement> =
        elements.iter().map(|e| (e.id.as_str(), *e)).collect();

    // Build node labels: container_id -> text
    let mut node_labels: HashMap<&str, &str> = HashMap::new();
    for el in elements {
        if let ElementData::Text {
            text,
            container_id: Some(cid),
            ..
        } = &el.element_data
        {
            node_labels.insert(cid.as_str(), text.as_str());
        }
    }

    // Build excalidraw_id -> readable_id mapping
    let mut id_map: HashMap<&str, String> = HashMap::new();
    let mut used_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let initial_word_count: usize = 3;
    for el in elements {
        let label = match &el.element_data {
            ElementData::Rectangle => node_labels.get(el.id.as_str()).copied().unwrap_or("?"),
            // Free-standing text (no container) can be an arrow endpoint
            ElementData::Text {
                text,
                container_id: None,
                ..
            } => text.as_str(),
            _ => continue,
        };
        let words = label_to_words(label);
        let take = initial_word_count.min(words.len());
        let mut candidate = words_to_id(&words[..take]);

        // Smart deduplication: extend with more words before numeric suffix
        if used_ids.contains(&candidate) {
            let mut n = take;
            while n < words.len() {
                n += 1;
                candidate = words_to_id(&words[..n]);
                if !used_ids.contains(&candidate) {
                    break;
                }
            }
            // If still colliding after all words exhausted, add numeric suffix
            if used_ids.contains(&candidate) {
                let base = candidate.clone();
                let mut counter = 2;
                loop {
                    candidate = format!("{}{}", base, counter);
                    if !used_ids.contains(&candidate) {
                        break;
                    }
                    counter += 1;
                }
            }
        }

        used_ids.insert(candidate.clone());
        id_map.insert(el.id.as_str(), candidate);
    }

    // Build edge labels: arrow_id -> text, and track which text elements are edge labels
    let mut edge_labels: HashMap<&str, &str> = HashMap::new();
    let mut edge_label_text_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for el in elements {
        match &el.element_data {
            ElementData::Arrow { .. } | ElementData::Line { .. } => {
                if let Some(bound) = &el.bound_elements {
                    for b in bound {
                        if b.kind == "text" {
                            if let Some(text_el) = index.get(b.id.as_str()) {
                                if let ElementData::Text { text, .. } = &text_el.element_data {
                                    edge_labels.insert(el.id.as_str(), text.as_str());
                                    edge_label_text_ids.insert(b.id.as_str());
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut out = String::new();
    writeln!(out, "flowchart TD").unwrap();

    // Build hierarchy and emit nodes with subgraph support
    let hierarchy = build_hierarchy(elements);
    let mut sg_counter: usize = 0;
    for node in &hierarchy.roots {
        emit_node(&mut out, node, &id_map, &node_labels, 1, &mut sg_counter);
    }

    // Emit free-standing text elements as nodes (skip edge labels)
    for el in elements {
        if let ElementData::Text {
            text,
            container_id: None,
            ..
        } = &el.element_data
        {
            if edge_label_text_ids.contains(el.id.as_str()) {
                continue;
            }
            if let Some(readable_id) = id_map.get(el.id.as_str()) {
                writeln!(out, "    {}[\"{}\"]", readable_id, escape_mermaid_text(text)).unwrap();
            }
        }
    }

    // Emit edges (arrows and lines)
    for el in elements {
        match &el.element_data {
            ElementData::Arrow {
                start_binding,
                end_binding,
                points,
            } => {
                let mut src = start_binding.as_ref().and_then(|b| id_map.get(b.element_id.as_str()));
                let mut dst = end_binding.as_ref().and_then(|b| id_map.get(b.element_id.as_str()));

                // Proximity fallback for unbound start
                if src.is_none() {
                    if let Some(first) = points.first() {
                        let px = el.x + first[0];
                        let py = el.y + first[1];
                        if let Some(nearest_id) = find_nearest_node(px, py, elements, &id_map) {
                            src = id_map.get(nearest_id);
                        }
                    }
                }

                // Proximity fallback for unbound end
                if dst.is_none() {
                    if let Some(last) = points.last() {
                        let px = el.x + last[0];
                        let py = el.y + last[1];
                        if let Some(nearest_id) = find_nearest_node(px, py, elements, &id_map) {
                            dst = id_map.get(nearest_id);
                        }
                    }
                }

                let label = edge_labels.get(el.id.as_str()).copied();
                emit_edge(&mut out, &el.id, src, dst, "-->", label);
            }
            ElementData::Line {
                start_binding,
                end_binding,
                ..
            } => {
                let src = start_binding.as_ref().and_then(|b| id_map.get(b.element_id.as_str()));
                let dst = end_binding.as_ref().and_then(|b| id_map.get(b.element_id.as_str()));
                let label = edge_labels.get(el.id.as_str()).copied();
                emit_edge(&mut out, &el.id, src, dst, "---", label);
            }
            _ => {}
        }
    }

    out
}

const PROXIMITY_THRESHOLD: f64 = 50.0;

/// Compute the distance from point (px, py) to the nearest edge of a rectangle.
/// Returns 0 if the point is exactly on the border, a positive value if outside,
/// and a negative-free interior distance (min to any edge) if inside.
fn dist_to_nearest_edge(px: f64, py: f64, left: f64, top: f64, right: f64, bottom: f64) -> f64 {
    let dx = (left - px).max(0.0_f64).max(px - right);
    let dy = (top - py).max(0.0_f64).max(py - bottom);
    if dx <= 0.0 && dy <= 0.0 {
        // Point is inside the rectangle: return min distance to any edge
        (px - left)
            .min(right - px)
            .min(py - top)
            .min(bottom - py)
    } else {
        (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt()
    }
}

/// Find the nearest node element to point (px, py) by distance to its border.
/// Among candidates within PROXIMITY_THRESHOLD, prefer the one with largest area
/// (outermost container) to avoid binding to a child rect when aiming at the container.
fn find_nearest_node<'a>(
    px: f64,
    py: f64,
    elements: &[&'a ExcalidrawElement],
    id_map: &HashMap<&str, String>,
) -> Option<&'a str> {
    let mut best: Option<(&'a str, f64, f64)> = None; // (id, distance, area)

    for el in elements {
        if !id_map.contains_key(el.id.as_str()) {
            continue;
        }
        let left = el.x;
        let top = el.y;
        let right = el.x + el.width;
        let bottom = el.y + el.height;
        let dist = dist_to_nearest_edge(px, py, left, top, right, bottom);
        if dist > PROXIMITY_THRESHOLD {
            continue;
        }
        let area = el.width * el.height;
        let dominated = match best {
            Some((_, bd, ba)) => dist < bd || (dist == bd && area > ba),
            None => true,
        };
        if dominated {
            best = Some((el.id.as_str(), dist, area));
        }
    }

    best.map(|(id, _, _)| id)
}

fn emit_edge(out: &mut String, id: &str, src: Option<&String>, dst: Option<&String>, connector: &str, label: Option<&str>) {
    match (src, dst) {
        (Some(s), Some(d)) => {
            match label {
                Some(l) => writeln!(out, "    {} {}|\"{}\"| {}", s, connector, escape_mermaid_text(l), d),
                None => writeln!(out, "    {} {} {}", s, connector, d),
            }
        }
        _ => {
            writeln!(out, "    %% edge {} has dangling binding", id)
        }
    }
    .unwrap();
}

fn emit_node(
    out: &mut String,
    node: &HierarchyNode,
    id_map: &HashMap<&str, String>,
    node_labels: &HashMap<&str, &str>,
    depth: usize,
    sg_counter: &mut usize,
) {
    let indent = "    ".repeat(depth);
    let readable_id = &id_map[node.element_id];

    if node.children.is_empty() {
        // Leaf node — regular Mermaid node
        let label = node_labels
            .get(node.element_id)
            .copied()
            .unwrap_or("?");
        writeln!(out, "{}{}[\"{}\"]", indent, readable_id, escape_mermaid_text(label)).unwrap();
    } else {
        // Container node — subgraph
        let label = node_labels.get(node.element_id).copied();
        let (sg_id, sg_label) = match label {
            Some(text) => (readable_id.clone(), format!("\"{}\"", escape_mermaid_text(text))),
            None => {
                *sg_counter += 1;
                (format!("sg_{}", sg_counter), "\" \"".to_string())
            }
        };
        writeln!(out, "{}subgraph {}[{}]", indent, sg_id, sg_label).unwrap();
        for child in &node.children {
            emit_node(out, child, id_map, node_labels, depth + 1, sg_counter);
        }
        writeln!(out, "{}end", indent).unwrap();
    }
}

/// Split a label into lowercase alphanumeric words.
fn label_to_words(label: &str) -> Vec<String> {
    label
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

/// Build a camelCase ID from a slice of words.
fn words_to_id(words: &[String]) -> String {
    if words.is_empty() {
        return "node".to_string();
    }
    let mut id = words[0].clone();
    for w in &words[1..] {
        let mut chars = w.chars();
        if let Some(first) = chars.next() {
            id.extend(first.to_uppercase());
            id.push_str(chars.as_str());
        }
    }
    if id.starts_with(|c: char| c.is_ascii_digit()) {
        format!("n{}", id)
    } else {
        id
    }
}

fn escape_mermaid_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("#quot;"),
            '[' => out.push_str("#lsqb;"),
            ']' => out.push_str("#rsqb;"),
            '(' => out.push_str("#lpar;"),
            ')' => out.push_str("#rpar;"),
            '{' => out.push_str("#lbrace;"),
            '}' => out.push_str("#rbrace;"),
            '<' => out.push_str("#lt;"),
            '>' => out.push_str("#gt;"),
            '\n' => out.push_str("<br>"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_to_words() {
        assert_eq!(label_to_words("Start"), vec!["start"]);
        assert_eq!(label_to_words("Hello World"), vec!["hello", "world"]);
        assert_eq!(label_to_words("123abc"), vec!["123abc"]);
        assert_eq!(label_to_words("a--b..c"), vec!["a", "b", "c"]);
        assert!(label_to_words("!!!").is_empty());
        assert!(label_to_words("").is_empty());
        assert_eq!(label_to_words("Process Data!"), vec!["process", "data"]);
    }

    #[test]
    fn test_words_to_id() {
        assert_eq!(words_to_id(&[]), "node");
        assert_eq!(words_to_id(&["start".into()]), "start");
        assert_eq!(words_to_id(&["hello".into(), "world".into()]), "helloWorld");
        assert_eq!(words_to_id(&["123abc".into()]), "n123abc");
        assert_eq!(
            words_to_id(&["customer".into(), "association".into(), "happens".into()]),
            "customerAssociationHappens"
        );
    }

    #[test]
    fn test_long_label_truncated_to_3_words() {
        let words = label_to_words("customer association happens by participant emails");
        assert_eq!(words.len(), 6);
        // First 3 words produce a short ID
        assert_eq!(words_to_id(&words[..3]), "customerAssociationHappens");
    }

    #[test]
    fn test_escape_mermaid_text() {
        assert_eq!(escape_mermaid_text("hello"), "hello");
        assert_eq!(escape_mermaid_text("a\"b"), "a#quot;b");
        assert_eq!(escape_mermaid_text("[test]"), "#lsqb;test#rsqb;");
        assert_eq!(escape_mermaid_text("line1\nline2"), "line1<br>line2");
        assert_eq!(escape_mermaid_text("f(x)"), "f#lpar;x#rpar;");
        assert_eq!(escape_mermaid_text("{a}"), "#lbrace;a#rbrace;");
        assert_eq!(escape_mermaid_text("<b>"), "#lt;b#gt;");
    }

    #[test]
    fn test_generate_simple_graph() {
        use crate::types::{Binding, BoundElement};

        let elements = vec![
            ExcalidrawElement {
                id: "rect1".into(),
                x: 0.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: Some(vec![BoundElement { id: "arrow1".into(), kind: "arrow".into() }]),
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "rect2".into(),
                x: 200.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: Some(vec![BoundElement { id: "arrow1".into(), kind: "arrow".into() }]),
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "text1".into(),
                x: 0.0, y: 0.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Start".into(),
                    original_text: "Start".into(),
                    container_id: Some("rect1".into()),
                },
            },
            ExcalidrawElement {
                id: "text2".into(),
                x: 200.0, y: 0.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "End".into(),
                    original_text: "End".into(),
                    container_id: Some("rect2".into()),
                },
            },
            ExcalidrawElement {
                id: "arrow1".into(),
                x: 100.0, y: 25.0, width: 100.0, height: 0.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Arrow {
                    points: vec![[0.0, 0.0], [100.0, 0.0]],
                    start_binding: Some(Binding { element_id: "rect1".into() }),
                    end_binding: Some(Binding { element_id: "rect2".into() }),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        assert!(output.starts_with("flowchart TD\n"));
        assert!(output.contains("start[\"Start\"]"));
        assert!(output.contains("end[\"End\"]"));
        assert!(output.contains("start --> end"));
    }

    #[test]
    fn test_generate_labeled_edge() {
        use crate::types::{Binding, BoundElement};

        let elements = vec![
            ExcalidrawElement {
                id: "r1".into(),
                x: 0.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "A".into(),
                    original_text: "A".into(),
                    container_id: Some("r1".into()),
                },
            },
            ExcalidrawElement {
                id: "r1".into(),
                x: 0.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "r2".into(),
                x: 200.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "r2_text".into(),
                x: 200.0, y: 0.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "B".into(),
                    original_text: "B".into(),
                    container_id: Some("r2".into()),
                },
            },
            ExcalidrawElement {
                id: "a1".into(),
                x: 100.0, y: 25.0, width: 100.0, height: 0.0,
                is_deleted: false,
                bound_elements: Some(vec![BoundElement { id: "t1".into(), kind: "text".into() }]),
                element_data: ElementData::Arrow {
                    points: vec![[0.0, 0.0], [100.0, 0.0]],
                    start_binding: Some(Binding { element_id: "r1".into() }),
                    end_binding: Some(Binding { element_id: "r2".into() }),
                },
            },
            ExcalidrawElement {
                id: "t1".into(),
                x: 150.0, y: 10.0, width: 40.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "proceed".into(),
                    original_text: "proceed".into(),
                    container_id: None,
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        assert!(output.contains("a -->|\"proceed\"| b"), "output was: {}", output);
    }

    #[test]
    fn test_collision_ids() {
        use crate::types::BoundElement;

        let elements = vec![
            ExcalidrawElement {
                id: "r1".into(),
                x: 0.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: Some(vec![BoundElement { id: "t1".into(), kind: "text".into() }]),
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "r2".into(),
                x: 200.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: Some(vec![BoundElement { id: "t2".into(), kind: "text".into() }]),
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "t1".into(),
                x: 0.0, y: 0.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Process".into(),
                    original_text: "Process".into(),
                    container_id: Some("r1".into()),
                },
            },
            ExcalidrawElement {
                id: "t2".into(),
                x: 200.0, y: 0.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Process".into(),
                    original_text: "Process".into(),
                    container_id: Some("r2".into()),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        assert!(output.contains("process[\"Process\"]"), "output was: {}", output);
        assert!(output.contains("process2[\"Process\"]"), "output was: {}", output);
    }

    #[test]
    fn test_subgraph_named_container() {
        let elements = vec![
            // Outer container rect
            ExcalidrawElement {
                id: "outer".into(),
                x: 0.0, y: 0.0, width: 500.0, height: 400.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            // Label for outer container
            ExcalidrawElement {
                id: "outer_text".into(),
                x: 10.0, y: 10.0, width: 100.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "My Group".into(),
                    original_text: "My Group".into(),
                    container_id: Some("outer".into()),
                },
            },
            // Inner leaf rect
            ExcalidrawElement {
                id: "inner".into(),
                x: 50.0, y: 50.0, width: 100.0, height: 80.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            // Label for inner rect
            ExcalidrawElement {
                id: "inner_text".into(),
                x: 60.0, y: 60.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Child".into(),
                    original_text: "Child".into(),
                    container_id: Some("inner".into()),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        assert!(output.contains("subgraph myGroup[\"My Group\"]"), "output was:\n{}", output);
        assert!(output.contains("child[\"Child\"]"), "output was:\n{}", output);
        assert!(output.contains("end"), "output was:\n{}", output);
    }

    #[test]
    fn test_subgraph_unnamed_container() {
        let elements = vec![
            // Unnamed outer container
            ExcalidrawElement {
                id: "outer".into(),
                x: 0.0, y: 0.0, width: 500.0, height: 400.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            // Inner leaf rect
            ExcalidrawElement {
                id: "inner".into(),
                x: 50.0, y: 50.0, width: 100.0, height: 80.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "inner_text".into(),
                x: 60.0, y: 60.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Leaf".into(),
                    original_text: "Leaf".into(),
                    container_id: Some("inner".into()),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        assert!(output.contains("subgraph sg_1[\" \"]"), "output was:\n{}", output);
        assert!(output.contains("leaf[\"Leaf\"]"), "output was:\n{}", output);
    }

    #[test]
    fn test_nested_subgraphs() {
        let elements = vec![
            ExcalidrawElement {
                id: "big".into(),
                x: 0.0, y: 0.0, width: 600.0, height: 500.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "big_text".into(),
                x: 10.0, y: 10.0, width: 80.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Outer".into(),
                    original_text: "Outer".into(),
                    container_id: Some("big".into()),
                },
            },
            ExcalidrawElement {
                id: "mid".into(),
                x: 50.0, y: 50.0, width: 300.0, height: 300.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "mid_text".into(),
                x: 60.0, y: 60.0, width: 80.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Middle".into(),
                    original_text: "Middle".into(),
                    container_id: Some("mid".into()),
                },
            },
            ExcalidrawElement {
                id: "leaf".into(),
                x: 100.0, y: 100.0, width: 80.0, height: 60.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "leaf_text".into(),
                x: 110.0, y: 110.0, width: 40.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Inner".into(),
                    original_text: "Inner".into(),
                    container_id: Some("leaf".into()),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        assert!(output.contains("subgraph outer[\"Outer\"]"), "output was:\n{}", output);
        assert!(output.contains("subgraph middle[\"Middle\"]"), "output was:\n{}", output);
        assert!(output.contains("inner[\"Inner\"]"), "output was:\n{}", output);
        // Should not contain inner as a subgraph
        assert!(!output.contains("subgraph inner"), "output was:\n{}", output);
    }

    #[test]
    fn test_cross_subgraph_edge() {
        use crate::types::Binding;

        let elements = vec![
            // Container
            ExcalidrawElement {
                id: "container".into(),
                x: 0.0, y: 0.0, width: 500.0, height: 400.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "container_text".into(),
                x: 10.0, y: 10.0, width: 80.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Group".into(),
                    original_text: "Group".into(),
                    container_id: Some("container".into()),
                },
            },
            // Node inside container
            ExcalidrawElement {
                id: "n1".into(),
                x: 50.0, y: 50.0, width: 100.0, height: 80.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "n1_text".into(),
                x: 60.0, y: 60.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "A".into(),
                    original_text: "A".into(),
                    container_id: Some("n1".into()),
                },
            },
            // Node outside container
            ExcalidrawElement {
                id: "n2".into(),
                x: 600.0, y: 50.0, width: 100.0, height: 80.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "n2_text".into(),
                x: 610.0, y: 60.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "B".into(),
                    original_text: "B".into(),
                    container_id: Some("n2".into()),
                },
            },
            // Arrow from n1 to n2 (cross-subgraph)
            ExcalidrawElement {
                id: "arrow1".into(),
                x: 150.0, y: 90.0, width: 450.0, height: 0.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Arrow {
                    points: vec![[0.0, 0.0], [450.0, 0.0]],
                    start_binding: Some(Binding { element_id: "n1".into() }),
                    end_binding: Some(Binding { element_id: "n2".into() }),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        assert!(output.contains("subgraph group[\"Group\"]"), "output was:\n{}", output);
        assert!(output.contains("a[\"A\"]"), "output was:\n{}", output);
        assert!(output.contains("b[\"B\"]"), "output was:\n{}", output);
        assert!(output.contains("a --> b"), "output was:\n{}", output);
    }

    #[test]
    fn test_proximity_fallback_arrow() {
        // Arrow with unbound start near a free-standing text node should resolve via proximity.
        // The text node is at (100, 100) with size 80x30.
        // Arrow starts at absolute position (95, 115) — just 5px to the left of the text border.
        // Arrow end is bound normally.
        use crate::types::Binding;

        let elements = vec![
            ExcalidrawElement {
                id: "text_node".into(),
                x: 100.0, y: 100.0, width: 80.0, height: 30.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Source".into(),
                    original_text: "Source".into(),
                    container_id: None,
                },
            },
            ExcalidrawElement {
                id: "rect_dest".into(),
                x: 400.0, y: 100.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "rect_dest_text".into(),
                x: 410.0, y: 110.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Dest".into(),
                    original_text: "Dest".into(),
                    container_id: Some("rect_dest".into()),
                },
            },
            ExcalidrawElement {
                id: "arrow_unbound".into(),
                // Arrow element position
                x: 95.0, y: 115.0, width: 355.0, height: 0.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Arrow {
                    // points[0] = [0,0] => absolute (95, 115), 5px left of text_node
                    // points[1] = [355,0] => absolute (450, 115), inside rect_dest
                    points: vec![[0.0, 0.0], [355.0, 0.0]],
                    start_binding: None, // unbound!
                    end_binding: Some(Binding { element_id: "rect_dest".into() }),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        // Should resolve the unbound start to "source" (the free-standing text)
        assert!(output.contains("source --> dest"), "expected proximity resolution, output was:\n{}", output);
        // Should NOT have a dangling comment
        assert!(!output.contains("dangling"), "should not have dangling binding, output was:\n{}", output);
    }

    #[test]
    fn test_proximity_fallback_prefers_largest_area() {
        // When an arrow endpoint is near both a container and its child,
        // prefer the container (larger area).
        use crate::types::Binding;

        let elements = vec![
            // Large container
            ExcalidrawElement {
                id: "container".into(),
                x: 0.0, y: 0.0, width: 400.0, height: 300.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "container_text".into(),
                x: 10.0, y: 10.0, width: 80.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Container".into(),
                    original_text: "Container".into(),
                    container_id: Some("container".into()),
                },
            },
            // Child rect sitting against left edge of container
            ExcalidrawElement {
                id: "child".into(),
                x: 10.0, y: 50.0, width: 100.0, height: 60.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "child_text".into(),
                x: 20.0, y: 60.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Child".into(),
                    original_text: "Child".into(),
                    container_id: Some("child".into()),
                },
            },
            // Target outside
            ExcalidrawElement {
                id: "target".into(),
                x: 500.0, y: 100.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "target_text".into(),
                x: 510.0, y: 110.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Target".into(),
                    original_text: "Target".into(),
                    container_id: Some("target".into()),
                },
            },
            // Arrow starting just outside the left edge of both container and child
            ExcalidrawElement {
                id: "arrow1".into(),
                x: -5.0, y: 80.0, width: 555.0, height: 0.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Arrow {
                    // Start at (-5, 80) — 5px from container border, ~15px from child border
                    points: vec![[0.0, 0.0], [555.0, 45.0]],
                    start_binding: None,
                    end_binding: Some(Binding { element_id: "target".into() }),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        // Should bind to container (larger area), not child
        // container is a subgraph, so its id is used for the edge
        assert!(output.contains("container --> target"), "expected binding to container, output was:\n{}", output);
    }

    #[test]
    fn test_line_does_not_get_proximity_fallback() {
        // Lines (decorative separators) should NOT get proximity fallback.
        let elements = vec![
            ExcalidrawElement {
                id: "rect1".into(),
                x: 0.0, y: 0.0, width: 200.0, height: 100.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "rect1_text".into(),
                x: 10.0, y: 10.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "Box".into(),
                    original_text: "Box".into(),
                    container_id: Some("rect1".into()),
                },
            },
            // Decorative line near the rect — should stay dangling
            ExcalidrawElement {
                id: "line1".into(),
                x: 5.0, y: 50.0, width: 190.0, height: 0.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Line {
                    points: vec![[0.0, 0.0], [190.0, 0.0]],
                    start_binding: None,
                    end_binding: None,
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        // Line should remain dangling — no proximity resolution
        assert!(output.contains("dangling"), "line should stay dangling, output was:\n{}", output);
    }

    #[test]
    fn test_smart_deduplication() {
        use crate::types::BoundElement;

        // Two nodes whose first 3 words collide: "good morning everyone" vs "good morning friends"
        // They should get distinct IDs without numeric suffixes.
        let elements = vec![
            ExcalidrawElement {
                id: "r1".into(),
                x: 0.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: Some(vec![BoundElement { id: "t1".into(), kind: "text".into() }]),
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "r2".into(),
                x: 200.0, y: 0.0, width: 100.0, height: 50.0,
                is_deleted: false,
                bound_elements: Some(vec![BoundElement { id: "t2".into(), kind: "text".into() }]),
                element_data: ElementData::Rectangle,
            },
            ExcalidrawElement {
                id: "t1".into(),
                x: 0.0, y: 0.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "good morning everyone today".into(),
                    original_text: "good morning everyone today".into(),
                    container_id: Some("r1".into()),
                },
            },
            ExcalidrawElement {
                id: "t2".into(),
                x: 200.0, y: 0.0, width: 50.0, height: 20.0,
                is_deleted: false,
                bound_elements: None,
                element_data: ElementData::Text {
                    text: "good morning friends forever".into(),
                    original_text: "good morning friends forever".into(),
                    container_id: Some("r2".into()),
                },
            },
        ];

        let refs: Vec<&ExcalidrawElement> = elements.iter().collect();
        let output = generate_mermaid(&refs);

        // First node gets 3 words: goodMorningEveryone
        assert!(output.contains("goodMorningEveryone["), "output was: {}", output);
        // Second node: "goodMorning" collides at 2 words, but at 3 words "goodMorningFriends" is unique
        assert!(output.contains("goodMorningFriends["), "output was: {}", output);
        // No numeric suffixes
        assert!(!output.contains("goodMorning2"), "output was: {}", output);
    }

    #[test]
    fn test_dist_to_nearest_edge() {
        // Outside — to the left
        assert!((dist_to_nearest_edge(-5.0, 50.0, 0.0, 0.0, 100.0, 100.0) - 5.0).abs() < 1e-9);
        // Outside — diagonal (bottom-right corner)
        let d = dist_to_nearest_edge(103.0, 104.0, 0.0, 0.0, 100.0, 100.0);
        assert!((d - 5.0).abs() < 0.1, "expected ~5.0, got {}", d);
        // Inside — closest edge is top (10px away)
        assert!((dist_to_nearest_edge(50.0, 10.0, 0.0, 0.0, 100.0, 100.0) - 10.0).abs() < 1e-9);
        // On the border
        assert!((dist_to_nearest_edge(0.0, 50.0, 0.0, 0.0, 100.0, 100.0) - 0.0).abs() < 1e-9);
    }
}
