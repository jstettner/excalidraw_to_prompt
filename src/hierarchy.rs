use std::collections::HashMap;

use crate::types::{ElementData, ExcalidrawElement};

const TOLERANCE: f64 = 20.0;

#[derive(Debug)]
pub struct HierarchyNode<'a> {
    pub element_id: &'a str,
    pub children: Vec<HierarchyNode<'a>>,
}

#[derive(Debug)]
pub struct Hierarchy<'a> {
    pub roots: Vec<HierarchyNode<'a>>,
}

struct Rect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    area: f64,
}

impl Rect {
    fn from_element(el: &ExcalidrawElement) -> Self {
        Rect {
            left: el.x,
            top: el.y,
            right: el.x + el.width,
            bottom: el.y + el.height,
            area: el.width * el.height,
        }
    }
}

fn contains_with_tolerance(outer: &Rect, inner: &Rect) -> bool {
    inner.area < outer.area
        && inner.left >= outer.left - TOLERANCE
        && inner.top >= outer.top - TOLERANCE
        && inner.right <= outer.right + TOLERANCE
        && inner.bottom <= outer.bottom + TOLERANCE
}

pub fn build_hierarchy<'a>(elements: &[&'a ExcalidrawElement]) -> Hierarchy<'a> {
    // Collect rectangles
    let rects: Vec<&ExcalidrawElement> = elements
        .iter()
        .filter(|e| matches!(e.element_data, ElementData::Rectangle))
        .copied()
        .collect();

    // Build rect geometry lookup
    let rect_map: HashMap<&str, Rect> = rects
        .iter()
        .map(|e| (e.id.as_str(), Rect::from_element(e)))
        .collect();

    // For each rect, find its parent = smallest containing rect
    let mut parent_map: HashMap<&str, &str> = HashMap::new();
    for child in &rects {
        let child_rect = &rect_map[child.id.as_str()];
        let mut best_parent: Option<&str> = None;
        let mut best_area = f64::MAX;

        for candidate in &rects {
            if candidate.id == child.id {
                continue;
            }
            let candidate_rect = &rect_map[candidate.id.as_str()];
            if contains_with_tolerance(candidate_rect, child_rect) && candidate_rect.area < best_area
            {
                best_parent = Some(candidate.id.as_str());
                best_area = candidate_rect.area;
            }
        }

        if let Some(pid) = best_parent {
            parent_map.insert(child.id.as_str(), pid);
        }
    }

    // Group children by parent
    let mut children_map: HashMap<&str, Vec<&str>> = HashMap::new();
    for rect in &rects {
        children_map.entry(rect.id.as_str()).or_default();
    }
    for (child_id, parent_id) in &parent_map {
        children_map.entry(parent_id).or_default().push(child_id);
    }

    // Build tree recursively
    fn build_node<'a>(
        id: &'a str,
        children_map: &HashMap<&str, Vec<&'a str>>,
    ) -> HierarchyNode<'a> {
        let children = children_map
            .get(id)
            .map(|kids| {
                kids.iter()
                    .map(|kid| build_node(kid, children_map))
                    .collect()
            })
            .unwrap_or_default();
        HierarchyNode {
            element_id: id,
            children,
        }
    }

    // Roots = rects with no parent
    let roots: Vec<HierarchyNode<'a>> = rects
        .iter()
        .filter(|r| !parent_map.contains_key(r.id.as_str()))
        .map(|r| build_node(r.id.as_str(), &children_map))
        .collect();

    Hierarchy { roots }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ExcalidrawElement;

    fn make_rect(id: &str, x: f64, y: f64, w: f64, h: f64) -> ExcalidrawElement {
        ExcalidrawElement {
            id: id.into(),
            x,
            y,
            width: w,
            height: h,
            is_deleted: false,
            bound_elements: None,
            element_data: ElementData::Rectangle,
        }
    }

    #[test]
    fn test_no_containment() {
        let r1 = make_rect("a", 0.0, 0.0, 100.0, 50.0);
        let r2 = make_rect("b", 200.0, 0.0, 100.0, 50.0);
        let elements: Vec<&ExcalidrawElement> = vec![&r1, &r2];
        let h = build_hierarchy(&elements);
        assert_eq!(h.roots.len(), 2);
        assert!(h.roots.iter().all(|n| n.children.is_empty()));
    }

    #[test]
    fn test_simple_containment() {
        let outer = make_rect("outer", 0.0, 0.0, 400.0, 300.0);
        let inner = make_rect("inner", 50.0, 50.0, 100.0, 80.0);
        let elements: Vec<&ExcalidrawElement> = vec![&outer, &inner];
        let h = build_hierarchy(&elements);
        assert_eq!(h.roots.len(), 1);
        assert_eq!(h.roots[0].element_id, "outer");
        assert_eq!(h.roots[0].children.len(), 1);
        assert_eq!(h.roots[0].children[0].element_id, "inner");
    }

    #[test]
    fn test_tolerance() {
        // Inner slightly exceeds outer bounds (within 20px tolerance)
        let outer = make_rect("outer", 100.0, 100.0, 400.0, 300.0);
        let inner = make_rect("inner", 90.0, 95.0, 200.0, 150.0);
        let elements: Vec<&ExcalidrawElement> = vec![&outer, &inner];
        let h = build_hierarchy(&elements);
        assert_eq!(h.roots.len(), 1);
        assert_eq!(h.roots[0].children.len(), 1);
    }

    #[test]
    fn test_tolerance_exceeded() {
        // Inner exceeds outer bounds by more than 20px
        let outer = make_rect("outer", 100.0, 100.0, 400.0, 300.0);
        let inner = make_rect("inner", 70.0, 70.0, 200.0, 150.0);
        let elements: Vec<&ExcalidrawElement> = vec![&outer, &inner];
        let h = build_hierarchy(&elements);
        assert_eq!(h.roots.len(), 2);
    }

    #[test]
    fn test_nested_containment() {
        let big = make_rect("big", 0.0, 0.0, 500.0, 500.0);
        let mid = make_rect("mid", 50.0, 50.0, 300.0, 300.0);
        let small = make_rect("small", 100.0, 100.0, 100.0, 100.0);
        let elements: Vec<&ExcalidrawElement> = vec![&big, &mid, &small];
        let h = build_hierarchy(&elements);
        assert_eq!(h.roots.len(), 1);
        assert_eq!(h.roots[0].element_id, "big");
        assert_eq!(h.roots[0].children.len(), 1);
        assert_eq!(h.roots[0].children[0].element_id, "mid");
        assert_eq!(h.roots[0].children[0].children.len(), 1);
        assert_eq!(h.roots[0].children[0].children[0].element_id, "small");
    }

    #[test]
    fn test_equal_size_not_contained() {
        let r1 = make_rect("a", 0.0, 0.0, 100.0, 100.0);
        let r2 = make_rect("b", 0.0, 0.0, 100.0, 100.0);
        let elements: Vec<&ExcalidrawElement> = vec![&r1, &r2];
        let h = build_hierarchy(&elements);
        assert_eq!(h.roots.len(), 2);
    }

    #[test]
    fn test_multiple_children() {
        let parent = make_rect("parent", 0.0, 0.0, 500.0, 500.0);
        let child1 = make_rect("c1", 10.0, 10.0, 100.0, 100.0);
        let child2 = make_rect("c2", 200.0, 10.0, 100.0, 100.0);
        let child3 = make_rect("c3", 10.0, 200.0, 100.0, 100.0);
        let elements: Vec<&ExcalidrawElement> = vec![&parent, &child1, &child2, &child3];
        let h = build_hierarchy(&elements);
        assert_eq!(h.roots.len(), 1);
        assert_eq!(h.roots[0].children.len(), 3);
    }
}
