use crate::Bundle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    I64,
    F32,
    F16,
    Other,
}

/// One declared graph input or output; negative dimensions are symbolic.
#[derive(Debug)]
pub struct Port<'graph> {
    pub name: &'graph str,
    pub element: Element,
    pub shape: &'graph [i64],
}

/// An admitted decoder convention: full-sequence logits and per-layer past/present tensors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub position_ids: bool,
    /// One element type, `F32` or `F16`, shared by every past and present tensor.
    pub cache: Element,
    /// `F32` or `F16`.
    pub logits: Element,
    /// Present outputs are written in place into preallocated past buffers.
    pub shared: bool,
}

/// Rejects graphs whose declared cache, dtype or dimensions differ from the bundle configuration.
/// Every input must be fed and every output is produced, so unknown names are incompatible.
///
/// # Errors
/// Returns a message naming the first incompatible port.
pub fn check(inputs: &[Port<'_>], outputs: &[Port<'_>], bundle: &Bundle) -> Result<Layout, String> {
    let find = |ports: &[Port<'_>], name: &str| ports.iter().position(|port| port.name == name);
    let cache = [-1, bundle.kv_heads as i64, -1, bundle.head_dim as i64];
    let element = inputs
        .iter()
        .find(|port| port.name == "past_key_values.0.key")
        .map(|port| port.element)
        .filter(|element| matches!(element, Element::F32 | Element::F16))
        .ok_or("onnx past_key_values.0.key must be a float32 or float16 tensor")?;
    let mut seen = vec![false; inputs.len()];
    for name in ["input_ids", "attention_mask"] {
        let index = find(inputs, name).ok_or(format!("onnx graph requires input {name}"))?;
        expect(&inputs[index], Element::I64, &[-1, -1])?;
        seen[index] = true;
    }
    let position_ids = find(inputs, "position_ids");
    if let Some(index) = position_ids {
        expect(&inputs[index], Element::I64, &[-1, -1])?;
        seen[index] = true;
    }
    for layer in 0..bundle.layers {
        for part in ["key", "value"] {
            let name = format!("past_key_values.{layer}.{part}");
            let index = find(inputs, &name).ok_or(format!("onnx graph requires input {name}"))?;
            expect(&inputs[index], element, &cache)?;
            seen[index] = true;
        }
    }
    if let Some(index) = seen.iter().position(|seen| !seen) {
        return Err(format!(
            "onnx graph input {} is not supported",
            inputs[index].name
        ));
    }
    let mut seen = vec![false; outputs.len()];
    let index = find(outputs, "logits").ok_or("onnx graph requires output logits")?;
    let logits = outputs[index].element;
    if !matches!(logits, Element::F32 | Element::F16) {
        return Err("onnx logits must be a float32 or float16 tensor".into());
    }
    expect(&outputs[index], logits, &[-1, -1, bundle.vocab as i64])?;
    seen[index] = true;
    for layer in 0..bundle.layers {
        for part in ["key", "value"] {
            let name = format!("present.{layer}.{part}");
            let index = find(outputs, &name).ok_or(format!("onnx graph requires output {name}"))?;
            expect(&outputs[index], element, &cache)?;
            seen[index] = true;
        }
    }
    if let Some(index) = seen.iter().position(|seen| !seen) {
        return Err(format!(
            "onnx graph output {} is not supported",
            outputs[index].name
        ));
    }
    Ok(Layout {
        position_ids: position_ids.is_some(),
        cache: element,
        logits,
        shared: bundle.shared,
    })
}

/// Fixed expected dimensions must match exactly; `-1` accepts any declared dimension.
fn expect(port: &Port<'_>, element: Element, shape: &[i64]) -> Result<(), String> {
    if port.element != element {
        return Err(format!(
            "onnx {} must be {element:?}, not {:?}",
            port.name, port.element
        ));
    }
    if port.shape.len() != shape.len()
        || port
            .shape
            .iter()
            .zip(shape)
            .any(|(declared, expected)| *expected >= 0 && *declared >= 0 && declared != expected)
    {
        return Err(format!(
            "onnx {} shape {:?} is incompatible with {shape:?}",
            port.name, port.shape
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn bundle() -> Bundle {
        Bundle {
            graph: PathBuf::new(),
            root: PathBuf::new(),
            context: 2048,
            layers: 2,
            kv_heads: 3,
            head_dim: 64,
            vocab: 256,
            eos: vec![2],
            shared: false,
        }
    }
    struct Graph {
        inputs: Vec<(String, Element, Vec<i64>)>,
        outputs: Vec<(String, Element, Vec<i64>)>,
    }
    impl Graph {
        fn new() -> Self {
            let mut inputs = vec![
                ("input_ids".into(), Element::I64, vec![-1, -1]),
                ("attention_mask".into(), Element::I64, vec![-1, -1]),
                ("position_ids".into(), Element::I64, vec![-1, -1]),
            ];
            let mut outputs = vec![("logits".into(), Element::F32, vec![-1, -1, 256])];
            for layer in 0..2 {
                for part in ["key", "value"] {
                    inputs.push((
                        format!("past_key_values.{layer}.{part}"),
                        Element::F32,
                        vec![-1, 3, -1, 64],
                    ));
                    outputs.push((
                        format!("present.{layer}.{part}"),
                        Element::F32,
                        vec![-1, 3, -1, 64],
                    ));
                }
            }
            Self { inputs, outputs }
        }
        fn check(&self) -> Result<Layout, String> {
            fn ports(list: &[(String, Element, Vec<i64>)]) -> Vec<Port<'_>> {
                list.iter()
                    .map(|(name, element, shape)| Port {
                        name,
                        element: *element,
                        shape,
                    })
                    .collect()
            }
            check(&ports(&self.inputs), &ports(&self.outputs), &bundle())
        }
    }
    #[test]
    fn optimum_decoder_signature_is_admitted() {
        assert_eq!(
            Graph::new().check(),
            Ok(Layout {
                position_ids: true,
                cache: Element::F32,
                logits: Element::F32,
                shared: false
            })
        );
    }
    #[test]
    fn position_ids_are_optional() {
        let mut graph = Graph::new();
        graph.inputs.retain(|(name, ..)| name != "position_ids");
        assert!(!graph.check().expect("layout").position_ids);
    }
    #[test]
    fn missing_layer_cache_is_rejected() {
        let mut graph = Graph::new();
        graph.outputs.retain(|(name, ..)| name != "present.1.value");
        assert!(graph.check().is_err());
    }
    #[test]
    fn mismatched_head_dimension_is_rejected() {
        let mut graph = Graph::new();
        graph.inputs[3].2 = vec![-1, 3, -1, 128];
        assert!(graph.check().is_err());
    }
    #[test]
    fn half_precision_cache_and_logits_are_admitted() {
        let mut graph = Graph::new();
        for (name, element, _) in graph.inputs.iter_mut().chain(graph.outputs.iter_mut()) {
            if name != "input_ids" && name != "attention_mask" && name != "position_ids" {
                *element = Element::F16;
            }
        }
        let layout = graph.check().expect("layout");
        assert_eq!((layout.cache, layout.logits), (Element::F16, Element::F16));
    }
    #[test]
    fn mixed_cache_precision_is_rejected() {
        let mut graph = Graph::new();
        graph.inputs[4].1 = Element::F16;
        assert!(graph.check().is_err());
    }
    #[test]
    fn unknown_input_is_rejected() {
        let mut graph = Graph::new();
        graph
            .inputs
            .push(("seqlens_k".into(), Element::I64, vec![-1]));
        assert!(graph.check().is_err());
    }
    #[test]
    fn mismatched_vocabulary_is_rejected() {
        let mut graph = Graph::new();
        graph.outputs[0].2 = vec![-1, -1, 512];
        assert!(graph.check().is_err());
    }
}
