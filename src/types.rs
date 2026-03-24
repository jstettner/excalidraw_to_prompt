use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ExcalidrawFile {
    pub elements: Vec<ExcalidrawElement>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcalidrawElement {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub is_deleted: bool,
    #[serde(default)]
    pub bound_elements: Option<Vec<BoundElement>>,
    #[serde(flatten)]
    pub element_data: ElementData,
}

#[derive(Debug, Deserialize)]
pub struct BoundElement {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ElementData {
    Rectangle,
    #[serde(rename_all = "camelCase")]
    Text {
        text: String,
        original_text: String,
        container_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Line {
        points: Vec<[f64; 2]>,
        start_binding: Option<Binding>,
        end_binding: Option<Binding>,
    },
    #[serde(rename_all = "camelCase")]
    Arrow {
        points: Vec<[f64; 2]>,
        start_binding: Option<Binding>,
        end_binding: Option<Binding>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub element_id: String,
}
