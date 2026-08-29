use std::fs;
use std::path::PathBuf;

use openapiv3::OpenAPI;
use serde_json::Value;

#[path = "openapi_normalize.rs"]
mod openapi_normalize;

use openapi_normalize::normalize_openapi_31_to_30;

fn main() {
    let spec_path = "openapi/openapi.json";
    println!("cargo:rerun-if-changed={spec_path}");
    println!("cargo:rerun-if-changed=openapi_normalize.rs");

    let raw = fs::read_to_string(spec_path).expect("failed to read OpenAPI schema");
    let mut value: Value = serde_json::from_str(&raw).expect("failed to parse OpenAPI JSON");

    normalize_openapi_31_to_30(&mut value);
    if let Value::Object(map) = &mut value {
        map.insert("openapi".to_string(), Value::String("3.0.3".to_string()));
    }

    let spec: OpenAPI = serde_json::from_value(value).expect("failed to deserialize OpenAPI model");

    let mut generator = progenitor::Generator::default();
    let tokens = generator
        .generate_tokens(&spec)
        .expect("failed to generate Rust client tokens");

    let ast = syn::parse2(tokens).expect("failed to parse generated token stream");
    let generated = prettyplease::unparse(&ast);

    let out_file = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR unavailable"))
        .join("indices_openapi.rs");

    fs::write(out_file, generated).expect("failed to write generated client source");
}
