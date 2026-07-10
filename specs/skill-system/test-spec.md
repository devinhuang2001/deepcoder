# Skill System Test Spec

## Unit Tests

- `skills_load_from_dir`
  - 准备：临时 skills 目录，包含一个 `SKILL.md`。
  - 执行：`load_from_dir`。
  - 断言：skills 数量为 1，name/description/instructions 正确。
  - Mock：临时目录。
- `skills_ignore_missing_dir`
  - 准备：不存在的 skills 目录。
  - 执行：`load_from_dir`。
  - 断言：返回 ok，skills 为空。
  - Mock：临时目录。
- `skills_parse_frontmatter_paths`
  - 准备：frontmatter 包含 `paths`。
  - 执行：解析 skill。
  - 断言：paths 被解析为列表。
  - Mock：临时文件。
- `skills_build_prompt_injection`
  - 准备：加载两个 skills。
  - 执行：`build_prompt_injection`。
  - 断言：输出包含标题、描述、instructions，顺序稳定。
  - Mock：临时目录。

## Integration Tests

- `skills_filter_by_active_paths`
  - 准备：一个 path-scoped skill 和一个 global skill。
  - 执行：传入 active path。
  - 断言：只返回匹配 skill 与 global skill。
  - Mock：临时目录。
- `engine_injects_active_skills`
  - 准备：engine 配置 skills dir。
  - 执行：构建 provider request。
  - 断言：system prompt 包含 active skill 内容。
  - Mock：mock provider。

## CLI/TUI Acceptance Tests

- `exec_uses_project_skill`
  - 准备：项目 `.deepcoder/skills/example/SKILL.md`。
  - 执行：`deepcoder exec "task"`。
  - 断言：mock provider 收到包含 skill injection 的 prompt。
  - Mock：mock provider、临时项目。

## Failure Cases

- `invalid_skill_warns_and_continues`
  - 准备：一个非法 skill 和一个合法 skill。
  - 执行：load dir。
  - 断言：合法 skill 仍加载，非法 skill 产生 warning。
  - Mock：临时目录。
- `duplicate_skill_names_deduped`
  - 准备：两个同名 skills。
  - 执行：构建 active skills。
  - 断言：prompt injection 不重复。
  - Mock：临时目录。
