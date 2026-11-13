use wasm_bindgen::prelude::*;

/// Compiles Tandem source to Lua, returning the generated Lua or an error.
#[wasm_bindgen]
pub fn compile(source: &str) -> Result<String, JsValue> {
    tandem::compiler::compile(source).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.severity.label(), diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
            .into()
    })
}
