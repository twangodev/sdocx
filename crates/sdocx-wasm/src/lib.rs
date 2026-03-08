use wasm_bindgen::prelude::*;

/// Parse a `.sdocx` file from bytes.
///
/// Accepts a `Uint8Array` and returns a `Document` object.
#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Result<JsValue, JsError> {
    let doc = sdocx::parse_bytes(bytes).map_err(|e| JsError::new(&e.to_string()))?;
    serde_wasm_bindgen::to_value(&doc).map_err(|e| JsError::new(&e.to_string()))
}