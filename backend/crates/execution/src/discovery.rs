use std::path::PathBuf;

/// 在 PATH 中搜索可执行文件
///
/// 跨平台处理：
/// - Windows：依次尝试 bare name、.cmd、.exe、.ps1 扩展名
/// - Unix：直接检查 `which <name>`
fn search_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let is_windows = cfg!(windows);

    for dir in std::env::split_paths(&path) {
        let base = dir.join(name);

        // 直接路径
        if base.is_file() {
            return Some(base);
        }

        // Windows：尝试常见扩展名
        if is_windows {
            for ext in &[".cmd", ".exe", ".bat", ".ps1"] {
                let with_ext = base.with_extension(ext.trim_start_matches('.'));
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }
    }

    None
}

/// 发现系统中可用的 coding agent
///
/// 返回一个列表，每项是 (agent_type, bin_path)。
/// 按优先级排序：claude-code > opencode
pub fn discover_agents() -> Vec<(&'static str, String)> {
    let mut agents = Vec::new();

    for &(agent_type, names) in &[
        ("claude-code", &["claude"][..]),
        ("opencode", &["opencode"][..]),
    ] {
        let found = names.iter().find_map(|name| search_in_path(name));
        if let Some(path) = found {
            let path_str = path.to_string_lossy().to_string();
            tracing::info!(agent_type = %agent_type, path = %path_str, "agent discovered");
            agents.push((agent_type, path_str));
        } else {
            tracing::warn!(agent_type = %agent_type, "agent not found on PATH, skipping");
        }
    }

    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_in_path_finds_self() {
        // Should find `cmd` on Windows or `sh` on Unix
        let name = if cfg!(windows) { "cmd" } else { "sh" };
        let result = search_in_path(name);
        assert!(result.is_some(), "should find {name} in PATH");
    }

    #[test]
    fn test_search_in_path_returns_none_for_nonsense() {
        let result = search_in_path("this_executable_does_not_exist_xyzzy");
        assert!(result.is_none());
    }
}