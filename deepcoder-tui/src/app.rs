//! TUI 应用状态

/// TUI 应用状态
pub struct App {
    pub messages: Vec<String>,
    pub input: String,
    pub reasoning: String,
    pub streaming: bool,
    pub show_reasoning: bool,
}

impl App {
    pub fn new(_config: deepcoder_config::Config) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            reasoning: String::new(),
            streaming: false,
            show_reasoning: true,
        }
    }
}
