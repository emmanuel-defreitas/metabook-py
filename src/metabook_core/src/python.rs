//! PyO3 bindings (feature "python"), imported as `metabook_core`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use crate::blocks::{Block, BlockType};
use crate::{Format, analyze, build_structure, textstats};

#[pyfunction]
fn split_sentences(text: &str) -> Vec<String> {
    textstats::split_sentences(text)
}

#[pyfunction]
fn count_sentences(text: &str) -> usize {
    textstats::count_sentences(text)
}

#[pyfunction]
fn count_words(text: &str) -> usize {
    textstats::count_words(text)
}

/// Parse a text/Markdown document and return the structure as JSON.
#[pyfunction]
fn analyze_json(py: Python<'_>, format: &str, text: &str) -> PyResult<String> {
    let format = Format::parse(format)
        .ok_or_else(|| PyValueError::new_err(format!("unsupported format: {format}")))?;
    let structure = py.detach(|| analyze(format, text));
    serde_json::to_string(&structure).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Block from a Python tuple `(type, text, level | None, children)`.
fn extract_block(obj: &Bound<'_, PyAny>) -> PyResult<Block> {
    let t = obj.cast::<PyTuple>()?;
    let kind: String = t.get_item(0)?.extract()?;
    let kind = BlockType::parse(&kind)
        .ok_or_else(|| PyValueError::new_err(format!("unknown block type: {kind}")))?;
    let text: String = t.get_item(1)?.extract()?;
    let level: Option<i64> = t.get_item(2)?.extract()?;
    let children_obj = t.get_item(3)?;
    let children_list = children_obj.cast::<PyList>()?;
    let mut children = Vec::with_capacity(children_list.len());
    for child in children_list.iter() {
        children.push(extract_block(&child)?);
    }
    Ok(Block {
        kind,
        text,
        level: level.map(|l| l.clamp(1, 6) as u8),
        children,
    })
}

/// Build the section tree for blocks produced by a Python adapter.
#[pyfunction]
fn build_structure_json(blocks: &Bound<'_, PyList>) -> PyResult<String> {
    let blocks = blocks
        .iter()
        .map(|b| extract_block(&b))
        .collect::<PyResult<Vec<_>>>()?;
    serde_json::to_string(&build_structure(blocks))
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

#[pymodule]
fn metabook_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(split_sentences, m)?)?;
    m.add_function(wrap_pyfunction!(count_sentences, m)?)?;
    m.add_function(wrap_pyfunction!(count_words, m)?)?;
    m.add_function(wrap_pyfunction!(analyze_json, m)?)?;
    m.add_function(wrap_pyfunction!(build_structure_json, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
