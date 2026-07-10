use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::runner::{self, DesktopRuntimeEvent};
use crate::state::{ChatMessage, ChatRole, DesktopAppState};

pub struct DesktopApp {
    pub state: DesktopAppState,
    runtime: tokio::runtime::Runtime,
    tx: mpsc::UnboundedSender<DesktopRuntimeEvent>,
    rx: mpsc::UnboundedReceiver<DesktopRuntimeEvent>,
    session: Option<deepcoder_engine::Session>,
    turn_handle: Option<JoinHandle<()>>,
}

impl DesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = egui::Rounding::same(12.0);
        visuals.window_fill = egui::Color32::from_rgba_premultiplied(30, 30, 30, 240);
        visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(20));
        visuals.panel_fill = egui::Color32::from_rgb(30, 30, 32);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(44, 44, 46);
        visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(60, 60, 65);
        visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(80, 80, 85);
        visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(10, 132, 255);
        visuals.widgets.active.rounding = egui::Rounding::same(8.0);
        visuals.selection.bg_fill = egui::Color32::from_rgb(10, 132, 255);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(10, 132, 255));

        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.window_margin = egui::Margin::same(16.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        cc.egui_ctx.set_style(style);
        cc.egui_ctx.set_visuals(visuals);

        let config = deepcoder_config::Config::load_default().unwrap_or_else(|_| {
            let mut config = deepcoder_config::Config {
                provider: Default::default(),
                sandbox: Default::default(),
                ui: Default::default(),
                system: Default::default(),
                api_key: None,
            };
            config.provider.model = "deepseek-chat".into();
            config
        });
        let (tx, rx) = mpsc::unbounded_channel();
        let mut state = DesktopAppState::new(
            config.clone(),
            std::env::var("DEEPSEEK_API_KEY").ok().as_deref(),
        );
        state.refresh_sessions();
        let session = runner::new_session(config, tx.clone());
        Self {
            state,
            runtime: tokio::runtime::Runtime::new().expect("tokio runtime"),
            tx,
            rx,
            session: Some(session),
            turn_handle: None,
        }
    }

    fn drain_runtime_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                DesktopRuntimeEvent::Engine(event) => self.state.apply_engine_event(event),
                DesktopRuntimeEvent::Approval(prompt) => self.state.queue_approval(prompt),
                DesktopRuntimeEvent::TurnFinished { session, result } => {
                    self.session = Some(*session);
                    self.turn_handle = None;
                    self.state.streaming = false;
                    self.state.refresh_sessions();
                    match result {
                        Ok(()) => self.state.status = "Ready".into(),
                        Err(error) => {
                            self.state.status = "Error".into();
                            self.state.messages.push(ChatMessage {
                                role: ChatRole::Error,
                                content: error,
                            });
                        }
                    }
                }
            }
        }
    }

    fn submit_current_input(&mut self) {
        if self.state.streaming || self.state.wizard.visible {
            return;
        }
        let input = self.state.input.trim().to_string();
        if input.is_empty() {
            return;
        }
        let Some(session) = self.session.take() else {
            self.state.status = "上一轮还在收尾".into();
            return;
        };
        self.state.input.clear();
        self.state.begin_user_message(input.clone());
        self.turn_handle = Some(runner::spawn_turn(
            &self.runtime,
            session,
            input,
            self.tx.clone(),
        ));
    }

    fn stop_turn(&mut self) {
        if let Some(handle) = self.turn_handle.take() {
            handle.abort();
        }
        self.session = Some(runner::new_session(
            self.state.config.clone(),
            self.tx.clone(),
        ));
        self.state.streaming = false;
        self.state.status = "已停止，已开启新会话".into();
    }

    fn new_chat(&mut self) {
        if self.state.streaming {
            self.stop_turn();
        }
        self.session = Some(runner::new_session(
            self.state.config.clone(),
            self.tx.clone(),
        ));
        self.state.messages.clear();
        self.state.reasoning.clear();
        self.state.tools.clear();
        self.state.status = "New chat".into();
    }

    fn load_selected_session(&mut self, fork: bool) {
        let Some(index) = self.state.selected_session else {
            return;
        };
        let Some(entry) = self.state.sessions.get(index) else {
            return;
        };
        let Ok(session_id) = entry.id.parse::<uuid::Uuid>() else {
            self.state.status = "session id 无效".into();
            return;
        };
        let persistence =
            deepcoder_persistence::Persistence::new(self.state.config.system.data_dir.clone());
        match runner::resume_session(
            self.state.config.clone(),
            &persistence,
            session_id,
            fork,
            self.tx.clone(),
        ) {
            Ok(Some(session)) => {
                self.state.load_messages_from_session(&session.messages);
                self.session = Some(session);
                self.state.refresh_sessions();
            }
            Ok(None) => self.state.status = "session 不存在".into(),
            Err(error) => self.state.status = format!("session 加载失败: {error}"),
        }
    }

    fn draw_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DeepCoder");
                ui.separator();
                ui.label(format!("模型: {}", self.state.config.provider.model));
                ui.separator();
                ui.label(format!("状态: {}", self.state.status));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("设置").clicked() {
                        self.state.wizard.visible = true;
                    }
                });
            });
        });
    }

    fn draw_sessions(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sessions")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("新建会话").clicked() {
                        self.new_chat();
                    }
                    if ui.button("刷新").clicked() {
                        self.state.refresh_sessions();
                    }
                });
                ui.separator();
                ui.label("历史会话");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut selected = self.state.selected_session;
                    for (index, session) in self.state.sessions.iter().enumerate() {
                        let title = session
                            .summary
                            .as_deref()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or(&session.model);
                        let label =
                            format!("{}  ·  {} 条", short_id(&session.id), session.message_count);
                        if ui
                            .selectable_label(selected == Some(index), label)
                            .on_hover_text(title)
                            .clicked()
                        {
                            selected = Some(index);
                        }
                    }
                    self.state.selected_session = selected;
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("恢复").clicked() {
                        self.load_selected_session(false);
                    }
                    if ui.button("分叉").clicked() {
                        self.load_selected_session(true);
                    }
                });
            });
    }

    fn draw_right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("details")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Reasoning");
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        if self.state.reasoning.is_empty() {
                            ui.weak("暂无推理内容");
                        } else {
                            ui.label(&self.state.reasoning);
                        }
                    });
                ui.separator();
                ui.heading("工具与审批");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for tool in &self.state.tools {
                        let color = if tool.is_error {
                            egui::Color32::LIGHT_RED
                        } else {
                            egui::Color32::LIGHT_GREEN
                        };
                        ui.colored_label(color, format!("{} · {}", tool.name, tool.status));
                        ui.small(&tool.detail);
                        ui.add_space(6.0);
                    }
                });
                ui.separator();
                ui.heading("Token");
                ui.label(format!("Input: {}", self.state.token_usage.input_tokens));
                ui.label(format!("Output: {}", self.state.token_usage.output_tokens));
                ui.label(format!(
                    "Reasoning: {}",
                    self.state.token_usage.reasoning_tokens
                ));
            });
    }

    fn draw_chat(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for message in &self.state.messages {
                        draw_chat_message(ui, message);
                        ui.add_space(8.0);
                    }
                });
        });
    }

    fn draw_input(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("input_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let send = ui
                    .add_enabled(
                        !self.state.streaming && !self.state.wizard.visible,
                        egui::Button::new("发送"),
                    )
                    .clicked();
                let stop = ui
                    .add_enabled(self.state.streaming, egui::Button::new("停止"))
                    .clicked();
                if send {
                    self.submit_current_input();
                }
                if stop {
                    self.stop_turn();
                }
                ui.weak("Ctrl+Enter 换行；点击发送提交");
            });
            let response = ui.add_enabled(
                !self.state.streaming && !self.state.wizard.visible,
                egui::TextEdit::multiline(&mut self.state.input)
                    .desired_rows(3)
                    .hint_text("输入你想让 DeepCoder 完成的任务..."),
            );
            if response.has_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter) && input.modifiers.ctrl)
            {
                self.state.input.push('\n');
            }
        });
    }

    fn draw_config_wizard(&mut self, ctx: &egui::Context) {
        if !self.state.wizard.visible {
            return;
        }
        egui::Window::new("首次配置")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("请填写 DeepSeek API Key，保存后即可开始聊天。");
                ui.add_space(8.0);
                ui.label("API Key");
                ui.add(
                    egui::TextEdit::singleline(&mut self.state.wizard.api_key)
                        .password(true)
                        .desired_width(420.0),
                );
                ui.label("模型");
                ui.text_edit_singleline(&mut self.state.wizard.model);
                ui.label("Base URL");
                ui.text_edit_singleline(&mut self.state.wizard.base_url);
                if let Some(error) = &self.state.wizard.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    if ui.button("保存配置").clicked()
                        && let Err(error) = self.state.save_wizard_config()
                    {
                        self.state.wizard.error = Some(error.to_string());
                    }
                    if std::env::var("DEEPSEEK_API_KEY").is_ok()
                        && ui.button("使用环境变量").clicked()
                    {
                        self.state.wizard.visible = false;
                        self.state.status = "使用环境变量 API Key".into();
                    }
                });
            });
    }

    fn draw_approval(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.state.pending_approval.as_ref() else {
            return;
        };
        let tool_name = prompt.tool_name.clone();
        let message = prompt.message.clone();
        let command = prompt.command.clone();
        let arguments = prompt.arguments.to_string();
        egui::Window::new("工具审批")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("工具: {tool_name}"));
                ui.label(message);
                if let Some(command) = command {
                    ui.separator();
                    ui.monospace(command);
                } else {
                    ui.separator();
                    ui.monospace(arguments);
                }
                ui.horizontal(|ui| {
                    if ui.button("批准").clicked() {
                        self.state.resolve_pending_approval(true);
                    }
                    if ui.button("拒绝").clicked() {
                        self.state.resolve_pending_approval(false);
                    }
                });
            });
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_runtime_events();
        self.draw_top_bar(ctx);
        self.draw_sessions(ctx);
        self.draw_right_panel(ctx);
        self.draw_chat(ctx);
        self.draw_input(ctx);
        self.draw_config_wizard(ctx);
        self.draw_approval(ctx);
        if self.state.streaming || self.state.pending_approval.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

fn draw_chat_message(ui: &mut egui::Ui, message: &ChatMessage) {
    let (title, color, bg_color) = match message.role {
        ChatRole::User => (
            "You",
            egui::Color32::from_rgb(200, 230, 255),
            egui::Color32::from_rgb(10, 132, 255),
        ),
        ChatRole::Assistant => (
            "DeepCoder",
            egui::Color32::LIGHT_GRAY,
            egui::Color32::from_rgb(44, 44, 46),
        ),
        ChatRole::System => (
            "System",
            egui::Color32::GRAY,
            egui::Color32::from_rgb(30, 30, 32),
        ),
        ChatRole::Error => (
            "Error",
            egui::Color32::WHITE,
            egui::Color32::from_rgb(255, 69, 58),
        ),
    };

    let frame = egui::Frame::none()
        .fill(bg_color)
        .inner_margin(egui::Margin::same(14.0))
        .rounding(egui::Rounding::same(12.0))
        .stroke(egui::Stroke::NONE);

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(
                egui::RichText::new(title).strong().color(color),
            ));
        });
        ui.add_space(6.0);
        ui.visuals_mut().override_text_color = Some(egui::Color32::from_rgb(245, 245, 245));
        draw_markdown_like(ui, &message.content);
    });
}

fn draw_markdown_like(ui: &mut egui::Ui, content: &str) {
    let mut in_code = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            ui.monospace(line);
        } else if in_code {
            ui.monospace(line);
        } else if line.starts_with('+') {
            ui.colored_label(egui::Color32::LIGHT_GREEN, line);
        } else if line.starts_with('-') {
            ui.colored_label(egui::Color32::LIGHT_RED, line);
        } else if trimmed.starts_with('#') {
            ui.heading(trimmed.trim_start_matches('#').trim());
        } else {
            ui.label(line);
        }
    }
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}
