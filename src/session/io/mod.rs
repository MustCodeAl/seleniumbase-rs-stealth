//! Session persistence and observability I/O adapters (`JsonLedger`,
//! `TracingEventSink`, atomic `identity_file`).

pub mod identity_file;
pub mod json_store;
pub mod tracing_sink;
