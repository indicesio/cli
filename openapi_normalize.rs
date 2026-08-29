use serde_json::{Map, Value};

pub fn normalize_openapi_31_to_30(node: &mut Value) {
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
            // OpenAPI 3.0 ignores sibling keywords of `$ref`. Wrapping the
            // reference in `allOf` is what lets `nullable: true` survive so
            // fields like `Run.error` deserialize as `Option<RunError>`.
            if single_case.contains_key("$ref") {
                map.insert(
                    "allOf".to_string(),
                    Value::Array(vec![Value::Object(single_case)]),
                );
            } else {
                for (k, v) in single_case {
                    map.entry(k).or_insert(v);
                }
            }
            map.insert("nullable".to_string(), Value::Bool(true));
            map.remove(key);
        }
    }
}
