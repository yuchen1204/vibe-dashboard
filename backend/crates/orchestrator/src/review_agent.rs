use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::llm::{self, ChatCompletionRequest, LlmConfig};
use crate::review::{self as review_repo, CreateFinding, FindingSeverity};

/// Review agent 事件——通过 channel 发送，让调用方实时推送到前端
#[derive(Debug, Clone)]
pub enum ReviewEvent {
    Started {
        review_id: String,
        job_id: String,
        todo_id: String,
    },
    Finding {
        review_id: String,
        finding: review_repo::ReviewFinding,
    },
    Completed {
        review_id: String,
        summary: String,
        score: i64,
        finding_count: i64,
    },
    Error {
        review_id: String,
        message: String,
    },
}

/// 运行 LLM 代码审查
///
/// 流程：
/// 1. 获取 job → todo → workspace 层级
/// 2. 执行 git diff 获取变更
/// 3. 调用 LLM 分析代码变更，输出结构化 findings
/// 4. 逐条写入 findings 并实时推送事件
/// 5. 更新 review summary + score
///
/// 如果 `review_id` 为 None，则自动创建新的 review 记录。
pub async fn run_review(
    pool: &SqlitePool,
    config: &LlmConfig,
    job_id: &str,
    todo_id: &str,
    review_id: Option<&str>,
    event_tx: Option<mpsc::UnboundedSender<ReviewEvent>>,
) -> Result<review_repo::ReviewDetail, String> {
    if !config.is_configured() {
        return Err("LLM 未配置。请先设置 API Key 来启用 AI 审查。".to_string());
    }

    // ---------- 1. 创建或使用已有 review 记录 ----------
    let review = if let Some(rid) = review_id {
        // 更新状态为 in_progress
        review_repo::update_review_status(pool, rid, review_repo::ReviewStatus::InProgress)
            .await
            .map_err(|e| format!("更新审查状态失败: {e}"))?;
        review_repo::get_review(pool, rid)
            .await
            .map_err(|e| format!("获取审查记录失败: {e}"))?
    } else {
        review_repo::create_review(pool, job_id, todo_id)
            .await
            .map_err(|e| format!("创建审查记录失败: {e}"))?
    };

    let review_id = review.id.clone();
    send_event(
        &event_tx,
        ReviewEvent::Started {
            review_id: review_id.clone(),
            job_id: job_id.to_string(),
            todo_id: todo_id.to_string(),
        },
    );

    // ---------- 3. 获取任务层级信息 ----------
    let todo = tasks::repo::get_todo(pool, todo_id)
        .await
        .map_err(|e| format!("获取 todo 失败: {e}"))?;
    let target = tasks::repo::get_target(pool, &todo.target_id)
        .await
        .map_err(|e| format!("获取 target 失败: {e}"))?;
    let ws = tasks::repo::get_workspace(pool, &target.workspace_id)
        .await
        .map_err(|e| format!("获取 workspace 失败: {e}"))?;

    let ws_path = std::path::Path::new(&ws.workspace.path);

    // ---------- 4. 获取 git diff ----------
    let diff = get_git_diff(ws_path).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to get git diff, review will proceed without diff context");
        String::new()
    });

    // ---------- 5. 构建 LLM prompt ----------
    let prompt = build_review_prompt(&todo.title, &todo.description, &diff);

    // ---------- 6. 调用 LLM ----------
    let api_messages = vec![
        llm::ChatCompletionMessage {
            role: "system".to_string(),
            content: Some(
                "You are a code review expert. Always respond with valid JSON only, no other text."
                    .to_string(),
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        llm::ChatCompletionMessage {
            role: "user".to_string(),
            content: Some(prompt),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    let request = ChatCompletionRequest {
        model: config.model.clone(),
        messages: api_messages,
        tools: None,
        tool_choice: None,
        max_tokens: config.max_tokens,
        temperature: 0.0,
    };

    let response = llm::chat_completion(config, request)
        .await
        .map_err(|e| format!("LLM 调用失败: {e}"))?;

    let content = response
        .choices
        .into_iter()
        .next()
        .ok_or("LLM 未返回有效回复")?
        .message
        .content
        .unwrap_or_default();

    // ---------- 7. 解析 LLM 响应 ----------
    let parsed = parse_review_response(&content)?;

    // ---------- 8. 逐条写入 findings ----------
    let mut total_findings: i64 = 0;
    for f in &parsed.findings {
        let severity = match f.severity.as_str() {
            "critical" => FindingSeverity::Critical,
            "major" => FindingSeverity::Major,
            "minor" => FindingSeverity::Minor,
            "suggestion" => FindingSeverity::Suggestion,
            _ => FindingSeverity::Minor,
        };

        let created = review_repo::add_finding(
            pool,
            &review_id,
            CreateFinding {
                severity,
                file_path: f.file_path.clone(),
                line_number: f.line_number,
                category: f.category.clone(),
                title: f.title.clone(),
                description: f.description.clone(),
                suggestion: f.suggestion.clone(),
            },
        )
        .await
        .map_err(|e| format!("写入 finding 失败: {e}"))?;

        total_findings += 1;
        send_event(
            &event_tx,
            ReviewEvent::Finding {
                review_id: review_id.clone(),
                finding: created,
            },
        );
    }

    // ---------- 9. 更新 review summary ----------
    let _ = review_repo::update_review_summary(
        pool,
        &review_id,
        &parsed.summary,
        Some(parsed.score),
        total_findings,
    )
    .await
    .map_err(|e| format!("更新审查总结失败: {e}"))?;

    // ---------- 10. 返回完整结果 ----------
    let detail = review_repo::get_review_with_findings(pool, &review_id)
        .await
        .map_err(|e| format!("获取审查详情失败: {e}"))?;

    send_event(
        &event_tx,
        ReviewEvent::Completed {
            review_id: review_id.clone(),
            summary: parsed.summary,
            score: parsed.score,
            finding_count: total_findings,
        },
    );

    Ok(detail)
}

/// 构建审查 prompt
fn build_review_prompt(title: &str, description: &str, diff: &str) -> String {
    // 截断 diff 避免超出 token 限制
    let diff_truncated = if diff.len() > 12000 {
        format!("{}\n\n... (diff 过长，仅显示前 12000 字符)", &diff[..12000])
    } else {
        diff.to_string()
    };

    format!(
        r#"你是一个代码审查专家。请审查以下代码变更，找出所有问题。

## 任务信息
**标题**: {title}
**描述**: {description}

## Git Diff
```
{diff}
```

请按以下 JSON 格式输出审查结果，不要包含其他内容：
{{
  "summary": "审查总结（用中文，2-5句话概括整体代码质量，指出主要问题和亮点）",
  "score": <1-10的整数评分>,
  "findings": [
    {{
      "severity": "critical|major|minor|suggestion",
      "file_path": "受影响的文件路径",
      "line_number": <行号或null>,
      "category": "bug|security|performance|style|maintainability|other",
      "title": "问题标题（简洁，用中文）",
      "description": "问题详细描述（用中文）",
      "suggestion": "修改建议（用中文）"
    }}
  ]
}}

要求：
- 如果没有发现问题，findings 为空数组，score 为 10
- 严重问题（critical）：可能导致崩溃、数据丢失、安全漏洞
- 主要问题（major）：逻辑错误、功能缺陷
- 次要问题（minor）：代码风格、命名不规范、缺少注释
- 建议（suggestion）：可优化的地方，性能提升，代码组织
- 最多输出 20 个 findings，按 severity 降序排列
- 如果 diff 为空，说明没有变更，score 为 10，findings 为空
- 必须返回合法 JSON，不要包含 markdown 代码块标记"#,
        title = title,
        description = description,
        diff = diff_truncated,
    )
}

/// 解析 LLM 返回的 JSON 审查结果
fn parse_review_response(content: &str) -> Result<ReviewResponse, String> {
    // 尝试直接解析
    let trimmed = content.trim();

    // 去掉可能的 markdown 代码块标记
    let cleaned = if trimmed.starts_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        let start = if lines[0].starts_with("```") { 1 } else { 0 };
        let end = if lines.last().map(|l| l.trim()).unwrap_or("") == "```" {
            lines.len().saturating_sub(1)
        } else {
            lines.len()
        };
        lines[start..end].join("\n")
    } else {
        trimmed.to_string()
    };

    serde_json::from_str::<ReviewResponse>(&cleaned)
        .map_err(|e| format!("解析 LLM 响应失败: {e}\n原始响应:\n{cleaned}"))
}

/// 获取 git diff（工作区中未提交的变更）
async fn get_git_diff(repo_path: &std::path::Path) -> Result<String, String> {
    // 检查是否是 git 仓库
    let is_git = tokio::process::Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(repo_path)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_git {
        return Err("not a git repository".to_string());
    }

    // 获取 staged + unstaged diff
    let output = tokio::process::Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| format!("git diff 失败: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff 执行失败: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // 如果 diff 为空，尝试获取未跟踪的文件
    if stdout.trim().is_empty() {
        // 获取未跟踪文件列表
        let untracked = tokio::process::Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(repo_path)
            .output()
            .await
            .map_err(|e| format!("git ls-files 失败: {e}"))?;

        let untracked_files = String::from_utf8_lossy(&untracked.stdout);
        let files: Vec<&str> = untracked_files.lines().collect();

        if files.is_empty() {
            return Ok(String::new());
        }

        // 读取未跟踪文件的内容作为 diff 的替代
        let mut content = String::from("(未跟踪的新文件):\n");
        for file in files.iter().take(10) {
            // 只读取文本文件，限制大小
            if let Ok(data) = tokio::fs::read_to_string(repo_path.join(file)).await {
                let lines: Vec<&str> = data.lines().collect();
                let preview = if lines.len() > 50 {
                    format!("{}...\n({} lines total)", lines[..50].join("\n"), lines.len())
                } else {
                    data.clone()
                };
                content.push_str(&format!("\n--- {file} ---\n{preview}\n"));
            }
        }
        if files.len() > 10 {
            content.push_str(&format!("\n... and {} more untracked files", files.len() - 10));
        }
        return Ok(content);
    }

    Ok(stdout)
}

fn send_event(tx: &Option<mpsc::UnboundedSender<ReviewEvent>>, event: ReviewEvent) {
    if let Some(ref tx) = tx {
        let _ = tx.send(event);
    }
}

// ---------- 解析用数据结构 ----------

#[derive(Debug, serde::Deserialize)]
struct ReviewResponse {
    summary: String,
    score: i64,
    #[serde(default)]
    findings: Vec<ReviewFindingResponse>,
}

#[derive(Debug, serde::Deserialize)]
struct ReviewFindingResponse {
    severity: String,
    file_path: String,
    line_number: Option<i64>,
    category: String,
    title: String,
    description: String,
    suggestion: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_review_json() {
        let json = r#"{
            "summary": "代码质量良好，有一个小问题。",
            "score": 8,
            "findings": [
                {
                    "severity": "minor",
                    "file_path": "src/main.rs",
                    "line_number": 42,
                    "category": "style",
                    "title": "未使用的变量",
                    "description": "变量 x 声明了但未使用",
                    "suggestion": "删除未使用的变量"
                }
            ]
        }"#;
        let result = parse_review_response(json).unwrap();
        assert_eq!(result.score, 8);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].severity, "minor");
    }

    #[test]
    fn parse_empty_findings() {
        let json = r#"{"summary": "完美", "score": 10, "findings": []}"#;
        let result = parse_review_response(json).unwrap();
        assert_eq!(result.score, 10);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn parse_with_markdown_fence() {
        let json = "```json\n{\"summary\": \"OK\", \"score\": 7, \"findings\": []}\n```";
        let result = parse_review_response(json).unwrap();
        assert_eq!(result.score, 7);
    }

    #[test]
    fn parse_invalid_json() {
        let result = parse_review_response("not json");
        assert!(result.is_err());
    }
}