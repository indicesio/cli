use std::collections::HashMap;
use std::time::Duration;

use opentelemetry::global;
use opentelemetry::propagation::Injector;
use opentelemetry::trace::{SpanContext, TracerProvider};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const SENTRY_TRACES_ENDPOINT: &str =
    "https://o109117.ingest.us.sentry.io/api/4511226766753792/integration/otlp/v1/traces";
const SENTRY_LOGS_ENDPOINT: &str =
    "https://o109117.ingest.us.sentry.io/api/4511226766753792/integration/otlp/v1/logs";
const SENTRY_AUTH_HEADER: &str = "sentry sentry_key=f10de9b5dda3541c5373f8934aab1894";

fn resource() -> Resource {
    Resource::builder()
        .with_service_name("indices-cli")
        .with_attribute(opentelemetry::KeyValue::new(
            opentelemetry_semantic_conventions::attribute::SERVICE_VERSION,
            env!("CARGO_PKG_VERSION"),
        ))
        .build()
}

fn init_tracer_provider(resource: Resource) -> SdkTracerProvider {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(SENTRY_TRACES_ENDPOINT)
        .with_headers(HashMap::from([(
            "x-sentry-auth".to_string(),
            SENTRY_AUTH_HEADER.to_string(),
        )]))
        .build()
        .expect("failed to build OTLP span exporter");

    SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build()
}

fn init_logger_provider(resource: Resource) -> SdkLoggerProvider {
    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .with_endpoint(SENTRY_LOGS_ENDPOINT)
        .with_headers(HashMap::from([(
            "x-sentry-auth".to_string(),
            SENTRY_AUTH_HEADER.to_string(),
        )]))
        .build()
        .expect("failed to build OTLP log exporter");

    SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build()
}

pub struct TelemetryGuard {
    tracer_provider: SdkTracerProvider,
    logger_provider: SdkLoggerProvider,
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        let _ = self.tracer_provider.force_flush();
        let _ = self.logger_provider.force_flush();
        let _ = self.tracer_provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT);
        let _ = self.logger_provider.shutdown_with_timeout(SHUTDOWN_TIMEOUT);
    }
}

pub fn init() -> TelemetryGuard {
    let resource = resource();
    let tracer_provider = init_tracer_provider(resource.clone());
    let logger_provider = init_logger_provider(resource);

    global::set_text_map_propagator(TraceContextPropagator::new());

    let tracer = tracer_provider.tracer("indices-cli");

    // Suppress noisy crates in logs/traces
    let log_filter = Targets::new()
        .with_default(tracing::Level::INFO)
        .with_target("hyper", tracing::Level::ERROR)
        .with_target("h2", tracing::Level::ERROR)
        .with_target("opentelemetry", tracing::Level::ERROR)
        .with_target("reqwest", tracing::Level::INFO);

    tracing_subscriber::registry()
        .with(OpenTelemetryLayer::new(tracer).with_filter(log_filter.clone()))
        .with(OpenTelemetryTracingBridge::new(&logger_provider).with_filter(log_filter))
        .init();

    TelemetryGuard {
        tracer_provider,
        logger_provider,
    }
}

/// Wrapper implementing `Injector` for `reqwest::header::HeaderMap`.
struct HeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(key.as_bytes()),
            reqwest::header::HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, val);
        }
    }
}

/// Formats a `SpanContext` into a `sentry-trace` header value: `{trace_id}-{span_id}-{sampled}`.
fn to_sentry_trace(span_context: &SpanContext) -> String {
    let trace_id = span_context.trace_id();
    let span_id = span_context.span_id();
    let sampled = if span_context.trace_flags().is_sampled() {
        '1'
    } else {
        '0'
    };
    format!("{trace_id}-{span_id}-{sampled}")
}

/// Injects Sentry distributed-tracing headers (`sentry-trace`, `baggage`) into the given headers.
pub fn inject_trace_context(headers: &mut reqwest::header::HeaderMap) {
    let context = tracing_opentelemetry::OpenTelemetrySpanExt::context(&tracing::Span::current());
    // Inject baggage via the global propagator (passes through any incoming baggage as-is).
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&context, &mut HeaderInjector(headers));
    });

    // Add Sentry's `sentry-trace` header for distributed tracing.
    use opentelemetry::trace::TraceContextExt;
    let span_context = context.span().span_context().clone();
    if span_context.is_valid() {
        if let Ok(val) =
            reqwest::header::HeaderValue::from_str(&to_sentry_trace(&span_context))
        {
            headers.insert("sentry-trace", val);
        }
    }
}
