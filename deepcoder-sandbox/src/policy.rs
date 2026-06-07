//! 执行策略 — PrefixRule 模式匹配

/// 执行策略决定
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyDecision {
    Allow,
    Deny(String),
    Prompt,
}

/// 前缀规则
#[derive(Debug, Clone)]
pub struct PrefixRule {
    pub prefix: String,
    pub allow: bool,
}

/// 执行策略引擎
pub struct ExecPolicy {
    rules: Vec<PrefixRule>,
    pub default_decision: PolicyDecision,
}

impl ExecPolicy {
    pub fn new() -> Self {
        Self {
            rules: vec![
                PrefixRule { prefix: "git".into(), allow: true },
                PrefixRule { prefix: "npm".into(), allow: true },
                PrefixRule { prefix: "pnpm".into(), allow: true },
                PrefixRule { prefix: "cargo".into(), allow: true },
                PrefixRule { prefix: "ls".into(), allow: true },
                PrefixRule { prefix: "cat".into(), allow: true },
                PrefixRule { prefix: "head".into(), allow: true },
                PrefixRule { prefix: "tail".into(), allow: true },
                PrefixRule { prefix: "echo".into(), allow: true },
                PrefixRule { prefix: "pwd".into(), allow: true },
                PrefixRule { prefix: "which".into(), allow: true },
                PrefixRule { prefix: "find".into(), allow: true },
                PrefixRule { prefix: "grep".into(), allow: true },
                PrefixRule { prefix: "curl".into(), allow: true },
                PrefixRule { prefix: "wget".into(), allow: true },
                PrefixRule { prefix: "mkdir".into(), allow: true },
                PrefixRule { prefix: "cp".into(), allow: true },
                PrefixRule { prefix: "mv".into(), allow: true },
            ],
            default_decision: PolicyDecision::Prompt,
        }
    }

    /// 评估命令
    pub fn evaluate(&self, command: &str) -> PolicyDecision {
        let trimmed = command.trim();

        for rule in &self.rules {
            if trimmed.starts_with(&rule.prefix) {
                return if rule.allow {
                    PolicyDecision::Allow
                } else {
                    PolicyDecision::Deny(format!("命令 '{rule:?}' 被策略禁止"))
                };
            }
        }

        self.default_decision.clone()
    }

    /// 添加规则
    pub fn add_rule(&mut self, prefix: impl Into<String>, allow: bool) {
        self.rules.push(PrefixRule { prefix: prefix.into(), allow });
    }
}
