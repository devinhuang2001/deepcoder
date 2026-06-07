//! DeepCoder 技能系统
//!
//! SKILL.md 加载、条件技能、模型注入

use std::path::Path;
use anyhow::Result;

/// 技能定义
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub paths: Option<Vec<String>>,
}

/// 技能管理器
pub struct SkillsManager {
    skills: Vec<Skill>,
}

impl SkillsManager {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    /// 从目录加载技能
    pub fn load_from_dir(&mut self, dir: &Path) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let skill_path = path.join("SKILL.md");
                if skill_path.exists() {
                    if let Ok(skill) = Self::parse_skill(&skill_path) {
                        self.skills.push(skill);
                    }
                }
            }
        }
        Ok(())
    }

    /// 解析 SKILL.md 文件
    fn parse_skill(path: &Path) -> Result<Skill> {
        let content = std::fs::read_to_string(path)?;

        // 解析 YAML 前置元数据
        let (frontmatter, instructions) = if let Some(rest) = content.trim_start().strip_prefix("---") {
            if let Some(end) = rest.find("---") {
                let yaml_part = &rest[..end];
                let body = rest[end + 3..].trim();
                (Self::parse_frontmatter(yaml_part), body.to_string())
            } else {
                (std::collections::HashMap::new(), content.trim().to_string())
            }
        } else {
            (std::collections::HashMap::new(), content.trim().to_string())
        };

        Ok(Skill {
            name: frontmatter.get("name").cloned().unwrap_or_default(),
            description: frontmatter.get("description").cloned().unwrap_or_default(),
            instructions,
            paths: frontmatter.get("paths").map(|p| p.split(',').map(|s| s.trim().to_string()).collect()),
        })
    }

    fn parse_frontmatter(yaml: &str) -> std::collections::HashMap<String, String> {
        yaml.lines()
            .filter_map(|line| {
                let mut parts = line.splitn(2, ':');
                match (parts.next(), parts.next()) {
                    (Some(key), Some(val)) => Some((key.trim().to_string(), val.trim().to_string())),
                    _ => None,
                }
            })
            .collect()
    }

    /// 获取所有技能（可选的路径过滤）
    pub fn get_active_skills(&self, _active_paths: &[String]) -> Vec<&Skill> {
        self.skills.iter().filter(|s| {
            s.paths.as_ref().map_or(true, |paths| {
                // 如果有 paths 限制，检查是否有匹配
                !paths.is_empty() // 简化：有 paths 就认为匹配
            })
        }).collect()
    }

    /// 构建注入到模型 prompt 的技能描述
    pub fn build_prompt_injection(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n\n## Available Skills\n");
        for skill in &self.skills {
            prompt.push_str(&format!("\n### {}\n{}\n", skill.name, skill.description));
            prompt.push_str(&format!("{}\n", skill.instructions));
        }
        prompt
    }
}
