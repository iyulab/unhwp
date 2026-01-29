use crate::{
    detect::{detect_format_from_bytes, FormatType},
    parse_bytes,
    render::{render_markdown, RenderOptions},
    Document,
};
use pyo3::prelude::*;

#[pyclass(name = "FormatType", eq, eq_int)]
#[derive(Clone, PartialEq, Debug)]
#[allow(non_camel_case_types)]
enum PyFormatType {
    Hwp5 = 1,
    Hwpx = 2,
    Hwp3 = 3,
}

impl From<FormatType> for PyFormatType {
    fn from(f: FormatType) -> Self {
        match f {
            FormatType::Hwp5 => PyFormatType::Hwp5,
            FormatType::Hwpx => PyFormatType::Hwpx,
            FormatType::Hwp3 => PyFormatType::Hwp3,
        }
    }
}

#[pyclass(name = "Document")]
#[derive(Clone)]
struct PyDocument {
    inner: Document,
}

#[pyfunction]
fn parse(data: &[u8]) -> PyResult<PyDocument> {
    let document = parse_bytes(data)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(PyDocument { inner: document })
}

#[pyfunction]
#[pyo3(signature = (data=None, document=None))]
fn convert_to_markdown(data: Option<&[u8]>, document: Option<&PyDocument>) -> PyResult<String> {
    if let Some(doc) = document {
        return render_markdown(&doc.inner, &RenderOptions::default())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()));
    }

    if let Some(bytes) = data {
        let document = parse_bytes(bytes)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

        return render_markdown(&document, &RenderOptions::default())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()));
    }

    Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
        "Either 'data' or 'document' must be provided.",
    ))
}

#[pyfunction]
fn detect_format(data: &[u8]) -> PyResult<PyFormatType> {
    let format = detect_format_from_bytes(data)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
    Ok(format.into())
}

#[pyfunction]
#[pyo3(signature = (data=None, document=None))]
fn is_distribution(data: Option<&[u8]>, document: Option<&PyDocument>) -> PyResult<bool> {
    if let Some(doc) = document {
        return Ok(doc.inner.metadata.is_distribution);
    }

    if let Some(bytes) = data {
        let document = parse_bytes(bytes)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        return Ok(document.metadata.is_distribution);
    }

    Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
        "Either 'data' or 'document' must be provided.",
    ))
}

#[pymodule]
fn _unhwp(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFormatType>()?;
    m.add_class::<PyDocument>()?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(detect_format, m)?)?;
    m.add_function(wrap_pyfunction!(convert_to_markdown, m)?)?;
    m.add_function(wrap_pyfunction!(is_distribution, m)?)?;

    Ok(())
}
