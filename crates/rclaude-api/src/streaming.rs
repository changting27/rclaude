use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest_eventsource::{Event, EventSource, RequestBuilderExt};

use crate::types::{ContentDelta, CreateMessageRequest, StreamEvent, Usage};
use rclaude_core::error::{RclaudeError, Result};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

/// A streaming response that yields StreamEvents.
pub struct MessageStream {
    inner: EventSource,
    done: bool,
}

/// High-level content event yielded during streaming.
#[derive(Debug, Clone)]
pub enum StreamContentEvent {
    /// A text delta was received.
    TextDelta { text: String },
    /// A thinking delta was received.
    ThinkingDelta { thinking: String },
    /// An input JSON delta for tool use.
    InputJsonDelta { partial_json: String },
    /// A content block started (with index and type info).
    ContentBlockStart {
        index: usize,
        block_type: String,
        /// For tool_use blocks: the tool use ID.
        tool_use_id: Option<String>,
        /// For tool_use blocks: the tool name.
        tool_name: Option<String>,
    },
    /// A content block finished.
    ContentBlockStop { index: usize },
    /// The message is complete.
    MessageComplete {
        stop_reason: Option<String>,
        usage: Option<Usage>,
    },
    /// An error occurred.
    Error { message: String },
}

/// Create a streaming message request.
pub fn create_stream(
    api_key: &str,
    base_url: Option<&str>,
    request: &CreateMessageRequest,
) -> Result<MessageStream> {
    let client = reqwest::Client::new();
    create_stream_with_client(
        &client,
        api_key,
        base_url.unwrap_or(DEFAULT_BASE_URL),
        request,
    )
}

/// Create a streaming message request reusing an existing HTTP client.
pub fn create_stream_with_client(
    client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
    request: &CreateMessageRequest,
) -> Result<MessageStream> {
    let url = format!("{base_url}/v1/messages");

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(api_key)
            .map_err(|_| RclaudeError::Config("Invalid API key".into()))?,
    );
    headers.insert("anthropic-version", HeaderValue::from_static(API_VERSION));

    let mut req = request.clone();
    req.stream = true;

    let body = serde_json::to_string(&req)?;

    let rb = client.post(&url).headers(headers).body(body);

    let es = rb.eventsource().map_err(|e| RclaudeError::Api {
        message: format!("Failed to create event source: {e}"),
        status: None,
    })?;

    Ok(MessageStream {
        inner: es,
        done: false,
    })
}

impl MessageStream {
    /// Process the next SSE event and return a high-level content event.
    pub async fn next_event(&mut self) -> Option<StreamContentEvent> {
        if self.done {
            return None;
        }

        loop {
            let event = self.inner.next().await;
            match event {
                Some(Ok(Event::Open)) => continue,
                Some(Ok(Event::Message(msg))) => {
                    let parsed: std::result::Result<StreamEvent, _> =
                        serde_json::from_str(&msg.data);

                    match parsed {
                        Ok(stream_event) => {
                            if let Some(content_event) = map_stream_event(stream_event) {
                                if matches!(
                                    content_event,
                                    StreamContentEvent::MessageComplete { .. }
                                ) {
                                    self.done = true;
                                }
                                return Some(content_event);
                            }
                            // Skip events we don't map (ping, etc)
                            continue;
                        }
                        Err(e) => {
                            // Try to continue on parse errors
                            tracing::warn!("Failed to parse SSE event: {e}");
                            continue;
                        }
                    }
                }
                Some(Err(e)) => {
                    self.done = true;
                    return Some(StreamContentEvent::Error {
                        message: format!("SSE error: {e}"),
                    });
                }
                None => {
                    self.done = true;
                    return None;
                }
            }
        }
    }

    /// Check if the stream is done.
    pub fn is_done(&self) -> bool {
        self.done
    }
}

fn map_stream_event(event: StreamEvent) -> Option<StreamContentEvent> {
    match event {
        StreamEvent::ContentBlockStart {
            index,
            content_block,
        } => {
            let (block_type, tool_use_id, tool_name) = match &content_block {
                crate::types::ApiContentBlock::Text { .. } => ("text", None, None),
                crate::types::ApiContentBlock::ToolUse { id, name, .. } => {
                    ("tool_use", Some(id.clone()), Some(name.clone()))
                }
                crate::types::ApiContentBlock::Thinking { .. } => ("thinking", None, None),
                _ => ("unknown", None, None),
            };
            Some(StreamContentEvent::ContentBlockStart {
                index,
                block_type: block_type.to_string(),
                tool_use_id,
                tool_name,
            })
        }
        StreamEvent::ContentBlockDelta { index: _, delta } => match delta {
            ContentDelta::TextDelta { text } => Some(StreamContentEvent::TextDelta { text }),
            ContentDelta::ThinkingDelta { thinking } => {
                Some(StreamContentEvent::ThinkingDelta { thinking })
            }
            ContentDelta::InputJsonDelta { partial_json } => {
                Some(StreamContentEvent::InputJsonDelta { partial_json })
            }
        },
        StreamEvent::ContentBlockStop { index } => {
            Some(StreamContentEvent::ContentBlockStop { index })
        }
        StreamEvent::MessageDelta { delta, usage } => Some(StreamContentEvent::MessageComplete {
            stop_reason: delta.stop_reason,
            usage,
        }),
        StreamEvent::MessageStop => Some(StreamContentEvent::MessageComplete {
            stop_reason: None,
            usage: None,
        }),
        StreamEvent::Error { error } => Some(StreamContentEvent::Error {
            message: format!("{}: {}", error.error_type, error.message),
        }),
        StreamEvent::Ping | StreamEvent::MessageStart { .. } => None,
    }
}

use futures::StreamExt;
