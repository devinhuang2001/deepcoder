//! DeepCoder 技能系统
//!
//! SKILL.md 加载、条件技能、模型注入。

use anyhow::Result;
use glob::Pattern;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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

        let mut entries = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_path = path.join("SKILL.md");
            if !skill_path.exists() {
                continue;
            }
            match Self::parse_skill(&skill_path) {
                Ok(skill) => self.skills.push(skill),
                Err(error) => {
                    tracing::warn!("failed to parse skill {}: {error}", skill_path.display())
                }
            }
        }
        self.skills
            .sort_by(|left, right| left.name.cmp(&right.name));
        Ok(())
    }

    /// 解析 SKILL.md 文件
    fn parse_skill(path: &Path) -> Result<Skill> {
        let content = std::fs::read_to_string(path)?;

        let (frontmatter, instructions) =
            if let Some(rest) = content.trim_start().strip_prefix("---") {
                if let Some(end) = rest.find("---") {
                    let yaml_part = &rest[..end];
                    let body = rest[end + 3..].trim();
                    (Self::parse_frontmatter(yaml_part), body.to_string())
                } else {
                    anyhow::bail!("frontmatter starts with --- but has no closing ---");
                }
            } else {
                (HashMap::new(), content.trim().to_string())
            };

        let name = frontmatter
            .get("name")
            .and_then(|values| values.first())
            .cloned()
            .or_else(|| {
                path.parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
            })
            .unwrap_or_default();
        if name.trim().is_empty() {
            anyhow::bail!("skill name is empty");
        }

        Ok(Skill {
            name,
            description: frontmatter
                .get("description")
                .and_then(|values| values.first())
                .cloned()
                .unwrap_or_default(),
            instructions,
            paths: frontmatter.get("paths").map(|paths| {
                paths
                    .iter()
                    .flat_map(|path| path.split(','))
                    .map(clean_yaml_value)
                    .filter(|path| !path.is_empty())
                    .collect()
            }),
        })
    }

    fn parse_frontmatter(yaml: &str) -> HashMap<String, Vec<String>> {
        let mut result: HashMap<String, Vec<String>> = HashMap::new();
        let mut current_key: Option<String> = None;
        for line in yaml.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(item) = trimmed.strip_prefix('-') {
                if let Some(key) = &current_key {
                    result
                        .entry(key.clone())
                        .or_default()
                        .push(clean_yaml_value(item));
                }
                continue;
            }

            let mut parts = trimmed.splitn(2, ':');
            let Some(key) = parts.next().map(str::trim).filter(|key| !key.is_empty()) else {
                continue;
            };
            let value = parts.next().map(str::trim).unwrap_or_default();
            current_key = Some(key.to_string());
            if value.is_empty() {
                result.entry(key.to_string()).or_default();
            } else if value.starts_with('[') && value.ends_with(']') {
                let values = value
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(clean_yaml_value)
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>();
                result.insert(key.to_string(), values);
            } else {
                result.insert(key.to_string(), vec![clean_yaml_value(value)]);
            }
        }
        result
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    /// 获取所有技能（可选的路径过滤）
    pub fn get_active_skills(&self, active_paths: &[String]) -> Vec<&Skill> {
        let mut seen = HashSet::new();
        self.skills
            .iter()
            .filter(|skill| seen.insert(skill.name.clone()))
            .filter(|skill| skill_matches_paths(skill, active_paths))
            .collect()
    }

    pub fn build_prompt_injection_for(skills: &[&Skill]) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n\n## Available Skills\n");
        let mut seen = HashSet::new();
        let mut ordered = skills.to_vec();
        ordered.sort_by(|left, right| left.name.cmp(&right.name));
        for skill in ordered {
            if !seen.insert(skill.name.as_str()) {
                continue;
            }
            prompt.push_str(&format!("\n### {}\n{}\n", skill.name, skill.description));
            prompt.push_str(&format!("{}\n", skill.instructions));
        }
        prompt
    }

    /// 构建注入到模型 prompt 的技能描述
    pub fn build_prompt_injection(&self) -> String {
        let skills = self.get_active_skills(&[]);
        Self::build_prompt_injection_for(&skills)
    }
}

impl Default for SkillsManager {
    fn default() -> Self {
        Self::new()
    }
}

fn clean_yaml_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

fn skill_matches_paths(skill: &Skill, active_paths: &[String]) -> bool {
    let Some(paths) = &skill.paths else {
        return true;
    };
    if paths.is_empty() {
        return true;
    }
    active_paths.iter().any(|active_path| {
        let normalized = active_path.replace('\\', "/");
        paths.iter().any(|pattern| {
            let normalized_pattern = pattern.replace('\\', "/");
            Pattern::new(&normalized_pattern)
                .map(|glob| glob.matches(&normalized))
                .unwrap_or_else(|_| normalized.contains(&normalized_pattern))
                || normalized.contains(&normalized_pattern)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("deepcoder_skills_{name}_{}", std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_skill(root: &Path, dir: &str, content: &str) {
        let path = root.join(dir);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn skills_load_from_dir() {
        let root = temp_dir("load");
        write_skill(
            &root,
            "rust",
            r#"---
name: rust
description: Rust help
---
Use cargo fmt.
"#,
        );
        let mut manager = SkillsManager::new();
        manager.load_from_dir(&root).unwrap();
        assert_eq!(manager.skills().len(), 1);
        assert_eq!(manager.skills()[0].name, "rust");
        assert!(manager.skills()[0].instructions.contains("cargo fmt"));
    }

    #[test]
    fn skills_ignore_missing_dir() {
        let mut manager = SkillsManager::new();
        manager
            .load_from_dir(&temp_dir("missing").join("none"))
            .unwrap();
        assert!(manager.skills().is_empty());
    }

    #[test]
    fn skills_parse_frontmatter_paths() {
        let root = temp_dir("paths");
        write_skill(
            &root,
            "frontend",
            r#"---
name: frontend
description: Frontend help
paths:
  - "src/**/*.tsx"
  - "web/**"
---
Use accessible controls.
"#,
        );
        let mut manager = SkillsManager::new();
        manager.load_from_dir(&root).unwrap();
        assert_eq!(
            manager.skills()[0].paths.as_ref().unwrap(),
            &vec!["src/**/*.tsx".to_string(), "web/**".to_string()]
        );
    }

    #[test]
    fn skills_filter_by_active_paths() {
        let root = temp_dir("filter");
        write_skill(
            &root,
            "global",
            r#"---
name: global
description: Global help
---
Always active.
"#,
        );
        write_skill(
            &root,
            "rust",
            r#"---
name: rust
description: Rust help
paths: ["src/**/*.rs"]
---
Rust active.
"#,
        );
        let mut manager = SkillsManager::new();
        manager.load_from_dir(&root).unwrap();

        let active = manager.get_active_skills(&["src/main.rs".into()]);
        assert!(active.iter().any(|skill| skill.name == "global"));
        assert!(active.iter().any(|skill| skill.name == "rust"));

        let active = manager.get_active_skills(&["README.md".into()]);
        assert!(active.iter().any(|skill| skill.name == "global"));
        assert!(!active.iter().any(|skill| skill.name == "rust"));
    }

    #[test]
    fn skills_build_prompt_injection_dedupes_and_sorts() {
        let root = temp_dir("prompt");
        write_skill(
            &root,
            "b",
            r#"---
name: same
description: First
---
First instructions.
"#,
        );
        write_skill(
            &root,
            "a",
            r#"---
name: alpha
description: Alpha
---
Alpha instructions.
"#,
        );
        let mut manager = SkillsManager::new();
        manager.load_from_dir(&root).unwrap();
        let prompt = manager.build_prompt_injection();
        assert!(prompt.find("### alpha").unwrap() < prompt.find("### same").unwrap());
        assert!(prompt.contains("Alpha instructions."));
    }

    #[test]
    fn invalid_skill_warns_and_continues() {
        let root = temp_dir("invalid");
        write_skill(&root, "bad", "---\nname: bad\nmissing close");
        write_skill(
            &root,
            "good",
            r#"---
name: good
description: Good
---
Good instructions.
"#,
        );
        let mut manager = SkillsManager::new();
        manager.load_from_dir(&root).unwrap();
        assert_eq!(manager.skills().len(), 1);
        assert_eq!(manager.skills()[0].name, "good");
    }
}
