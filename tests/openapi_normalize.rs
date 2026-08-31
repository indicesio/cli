#[path = "../openapi_normalize.rs"]
mod openapi_normalize;

use openapi_normalize::normalize_openapi_31_to_30;
use serde_json::{Value, json};

#[test]
fn wraps_nullable_schema_ref_in_allof() {
    let mut spec = json!({
        "error": {
            "anyOf": [
                { "$ref": "#/components/schemas/RunError" },
                { "type": "null" }
            ],
            "description": "Why the run failed."
        }
    });

    normalize_openapi_31_to_30(&mut spec);

    assert_eq!(
        spec["error"],
        json!({
            "allOf": [{ "$ref": "#/components/schemas/RunError" }],
            "nullable": true,
            "description": "Why the run failed."
        })
    );
}

#[test]
fn converts_nullable_primitive_anyof_to_nullable_type() {
    let mut spec = json!({
        "exception": {
            "anyOf": [
                { "type": "string" },
                { "type": "null" }
            ],
            "description": "Exception class name."
        }
    });

    normalize_openapi_31_to_30(&mut spec);

    assert_eq!(
        spec["exception"],
        json!({
            "type": "string",
            "nullable": true,
            "description": "Exception class name."
        })
    );
}

#[test]
fn converts_nullable_object_anyof_to_nullable_type() {
    let mut spec = json!({
        "result": {
            "anyOf": [
                {
                    "additionalProperties": true,
                    "type": "object"
                },
                { "type": "null" }
            ],
            "description": "Execution result of the run."
        }
    });

    normalize_openapi_31_to_30(&mut spec);

    assert_eq!(
        spec["result"],
        json!({
            "additionalProperties": true,
            "type": "object",
            "nullable": true,
            "description": "Execution result of the run."
        })
    );
}

#[test]
fn converts_type_array_null_union() {
    let mut spec = json!({
        "retryable": {
            "type": ["boolean", "null"]
        }
    });

    normalize_openapi_31_to_30(&mut spec);

    assert_eq!(
        spec["retryable"],
        json!({
            "type": "boolean",
            "nullable": true
        })
    );
}

#[test]
fn leaves_non_nullable_anyof_untouched() {
    let mut spec = json!({
        "body": {
            "anyOf": [
                { "$ref": "#/components/schemas/StartCaptureSessionRequest" },
                { "type": "string" }
            ]
        }
    });
    let original = spec.clone();

    normalize_openapi_31_to_30(&mut spec);

    assert_eq!(spec, original);
}

#[test]
fn normalizes_run_error_field_from_production_spec() {
    let raw = include_str!("../openapi/openapi.json");
    let mut spec: Value = serde_json::from_str(raw).expect("openapi snapshot should parse");

    normalize_openapi_31_to_30(&mut spec);

    let error = &spec["components"]["schemas"]["Run"]["properties"]["error"];
    assert_eq!(
        error["allOf"],
        json!([{ "$ref": "#/components/schemas/RunError" }])
    );
    assert_eq!(error["nullable"], json!(true));
    assert!(error.get("anyOf").is_none());
    assert!(error.get("$ref").is_none());
}

#[test]
fn normalizes_run_result_field_from_production_spec() {
    let raw = include_str!("../openapi/openapi.json");
    let mut spec: Value = serde_json::from_str(raw).expect("openapi snapshot should parse");

    normalize_openapi_31_to_30(&mut spec);

    let properties = &spec["components"]["schemas"]["Run"]["properties"];
    assert!(properties.get("result_json").is_none());

    let result = &properties["result"];
    assert_eq!(result["type"], json!("object"));
    assert_eq!(result["additionalProperties"], json!(true));
    assert_eq!(result["nullable"], json!(true));
    assert!(result.get("anyOf").is_none());
}
