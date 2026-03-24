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
    let mut used_ids: HashMap<String, usize> = HashMap::new();
    for el in elements {
        if let ElementData::Rectangle = &el.element_data {
            let label = node_labels.get(el.id.as_str()).copied().unwrap_or("?");
            let base = label_to_id(label);
            let count = used_ids.entry(base.clone()).or_insert(0);
            *count += 1;
            let readable = if *count == 1 {
                base
            } else {
                format!("{}_{}", base, count)
            };
            id_map.insert(el.id.as_str(), readable);
        }
    }

    // Build edge labels: arrow_id -> text
    let mut edge_labels: HashMap<&str, &str> = HashMap::new();
    for el in elements {
        match &el.element_data {
            ElementData::Arrow { .. } | ElementData::Line { .. } => {
                if let Some(bound) = &el.bound_elements {
                    for b in bound {
                        if b.kind == "text" {
                            if let Some(text_el) = index.get(b.id.as_str()) {
                                if let ElementData::Text { text, .. } = &text_el.element_data {
                                    edge_labels.insert(el.id.as_str(), text.as_str());
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

    // Emit edges (arrows and lines)
    for el in elements {
        match &el.element_data {
            ElementData::Arrow {
                start_binding,
                end_binding,
                ..
            } => {
                let src = start_binding.as_ref().and_then(|b| id_map.get(b.element_id.as_str()));
                let dst = end_binding.as_ref().and_then(|b| id_map.get(b.element_id.as_str()));
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

fn label_to_id(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    // Collapse consecutive underscores
    let mut collapsed = String::new();
    let mut prev_underscore = false;
    for c in out.chars() {
        if c == '_' {
            if !prev_underscore {
                collapsed.push('_');
            }
            prev_underscore = true;
        } else {
            collapsed.push(c);
            prev_underscore = false;
        }
    }
    // Trim trailing underscores
    let trimmed = collapsed.trim_end_matches('_');
    if trimmed.is_empty() {
        return "node".to_string();
    }
    // Prepend 'n' if starts with digit
    if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
        format!("n{}", trimmed)
    } else {
        trimmed.to_string()
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
    fn test_label_to_id() {
        assert_eq!(label_to_id("Start"), "start");
        assert_eq!(label_to_id("Hello World"), "hello_world");
        assert_eq!(label_to_id("123abc"), "n123abc");
        assert_eq!(label_to_id("a--b..c"), "a_b_c");
        assert_eq!(label_to_id("!!!"), "node");
        assert_eq!(label_to_id(""), "node");
        assert_eq!(label_to_id("Process Data!"), "process_data");
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
        assert!(output.contains("process_2[\"Process\"]"), "output was: {}", output);
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

        assert!(output.contains("subgraph my_group[\"My Group\"]"), "output was:\n{}", output);
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
}
