//! Output styles: Default, Explanatory, and Learning modes.
//! Supports Default, Explanatory, and Learning modes.

/// Output style configuration.
#[derive(Debug, Clone)]
pub struct OutputStyleConfig {
    pub name: &'static str,
    pub description: &'static str,
    pub prompt: &'static str,
}

pub const STYLE_EXPLANATORY: OutputStyleConfig = OutputStyleConfig {
    name: "Explanatory",
    description: "Provides educational insights alongside actions",
    prompt: "After completing each action, briefly explain the reasoning behind your approach \
             and any relevant concepts. Include 'why' explanations that help the user learn, \
             not just 'what' you did. Keep explanations concise but educational.",
};

pub const STYLE_LEARNING: OutputStyleConfig = OutputStyleConfig {
    name: "Learning",
    description: "Hands-on practice with guided exercises",
    prompt: "After completing tasks, suggest a brief hands-on exercise the user could try to \
             reinforce the concepts involved. Frame it as 'Try this:' followed by a concrete, \
             small task they can do immediately. Keep suggestions practical and directly related \
             to what was just done.",
};

/// Get all available output styles.
pub fn get_output_styles() -> Vec<&'static OutputStyleConfig> {
    vec![&STYLE_EXPLANATORY, &STYLE_LEARNING]
}

/// Get the system prompt addition for an output style.
pub fn get_output_style_prompt(style_name: &str) -> Option<&'static str> {
    match style_name.to_lowercase().as_str() {
        "explanatory" => Some(STYLE_EXPLANATORY.prompt),
        "learning" => Some(STYLE_LEARNING.prompt),
        "default" | "" => None,
        _ => None,
    }
}
