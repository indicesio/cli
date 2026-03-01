use std::fs;
use std::path::PathBuf;

use openapiv3::OpenAPI;
use serde_json::{Map, Value};

fn main() {
    let spec_path = "openapi/openapi.json";
    println!("cargo:rerun-if-changed={spec_path}");

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

fn normalize_openapi_31_to_30(node: &mut Value) {
    match node {
        Value::Object(map) => normalize_object(map),
        Value::Array(items) => {
            for item in items {
                normalize_openapi_31_to_30(item);
            }
        }
        _ => {}
    }
}

fn normalize_object(map: &mut Map<String, Value>) {
    for value in map.values_mut() {
        normalize_openapi_31_to_30(value);
    }

    if let Some(value_type) = map.get_mut("type") {
        if let Value::Array(types) = value_type {
            let mut non_null = Vec::new();
            let mut has_null = false;
            for item in types {
                if let Value::String(kind) = item {
                    if kind == "null" {
                        has_null = true;
                    } else {
                        non_null.push(kind.clone());
                    }
                }
            }

            if has_null && non_null.len() == 1 {
                *value_type = Value::String(non_null.pop().expect("single non-null type"));
                map.insert("nullable".to_string(), Value::Bool(true));
            }
        }
    }

    convert_nullable_combinators(map, "anyOf");
    convert_nullable_combinators(map, "oneOf");
}

fn convert_nullable_combinators(map: &mut Map<String, Value>, key: &str) {
    let Some(Value::Array(cases)) = map.get_mut(key) else {
        return;
    };

    let mut has_null = false;
    let mut remaining = Vec::new();

    for case in cases.iter() {
        match case {
            Value::Object(obj) if matches!(obj.get("type"), Some(Value::String(kind)) if kind == "null") =>
            {
                has_null = true;
            }
            _ => remaining.push(case.clone()),
        }
    }

    if has_null && remaining.len() == 1 {
        if let Value::Object(single_case) = remaining.pop().expect("single non-null case") {
            for (k, v) in single_case {
                map.entry(k).or_insert(v);
            }
            map.insert("nullable".to_string(), Value::Bool(true));
            map.remove(key);
        }
    }
}
