pub mod chat;
pub mod gemini;
pub mod responses;

// Note: spawn_gemini_stream is available but currently unused since most gateways
// don't support Gemini's streamGenerateContent endpoint. The gemini module is kept
// for future use when streaming support is available.
#[allow(unused_imports)]
pub(crate) use gemini::spawn_gemini_stream;
pub use responses::process_sse;
pub use responses::spawn_response_stream;
pub use responses::stream_from_fixture;
