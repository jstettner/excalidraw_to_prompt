use std::collections::HashMap;
use std::fmt::Write;

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

    // Emit nodes (rectangles)
    for el in elements {
        if let ElementData::Rectangle = &el.element_data {
            let label = node_labels.get(el.id.as_str()).copied().unwrap_or("?");
            let readable_id = &id_map[el.id.as_str()];
            writeln!(
                out,
                "    {}[\"{}\"]",
                readable_id,
                escape_mermaid_text(label)
            )
            .unwrap();
        }
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
}
