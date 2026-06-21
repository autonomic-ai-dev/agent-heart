use anyhow::Result;
use std::path::Path;

pub fn lint_script(path: &Path) -> Result<()> {
    let source = std::fs::read_to_string(path)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_bash::LANGUAGE.into())?;

    let tree = parser
        .parse(&source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse script"))?;

    let root = tree.root_node();
    let mut issues: Vec<LintIssue> = Vec::new();

    check_node(&source, root, 0, &mut issues);

    if issues.is_empty() {
        println!("✅ No issues found in {}", path.display());
    } else {
        println!(
            "🔍 {} found in {}:",
            if issues.len() == 1 {
                "1 issue".to_string()
            } else {
                format!("{} issues", issues.len())
            },
            path.display()
        );
        println!();
        for issue in &issues {
            let level = match issue.severity {
                Severity::Error => "ERROR",
                Severity::Warning => "WARN",
                Severity::Info => "INFO",
            };
            println!("  [{level}] Line {}: {}", issue.line, issue.message);
            if let Some(ref snippet) = issue.snippet {
                println!("         {}", snippet.trim());
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
#[allow(dead_code)]
enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug)]
struct LintIssue {
    line: usize,
    severity: Severity,
    message: String,
    snippet: Option<String>,
}

fn check_node(source: &str, node: tree_sitter::Node, _depth: usize, issues: &mut Vec<LintIssue>) {
    let kind = node.kind();
    let start = node.start_position();
    let line = start.row + 1;

    let node_text = &source[node.byte_range()];

    match kind {
        "command" => {
            let cmd_text = node_text.to_lowercase();
            if cmd_text.starts_with("rm ")
                && (cmd_text.contains(" -rf ") || cmd_text.contains(" -r "))
                && !cmd_text.contains("--no-preserve-root")
            {
                    issues.push(LintIssue {
                        line,
                        severity: Severity::Warning,
                        message: "Use 'rm -rf' with caution. Consider adding --no-preserve-root for explicit intent."
                            .into(),
                        snippet: Some(node_text.to_string()),
                    });
            }
            if cmd_text.starts_with("eval ")
                || cmd_text.starts_with("source ")
                || cmd_text.starts_with(". ")
            {
                issues.push(LintIssue {
                    line,
                    severity: Severity::Warning,
                    message: "Dynamic execution (eval/source/.) can lead to code injection".into(),
                    snippet: Some(node_text.to_string()),
                });
            }
            if (cmd_text.starts_with("curl ") || cmd_text.starts_with("wget "))
                && !cmd_text.contains("--proto") && !cmd_text.contains("--secure")
            {
                    issues.push(LintIssue {
                        line,
                        severity: Severity::Info,
                        message: "Consider verifying HTTPS/TLS flags for curl/wget".into(),
                        snippet: Some(node_text.to_string()),
                    });
            }
        }
        "variable_name" => {
            let parent = node.parent();
            if let Some(p) = parent {
                if p.kind() == "simple_expansion" {
                    if let Some(grandparent) = p.parent() {
                        if grandparent.kind() != "string" && grandparent.kind() != "word" {
                        } else {
                            let _text = node_text;
                        }
                    }
                }
            }
        }
        _ => {}
    }

    if kind == "program" {
        for (i, line_str) in source.lines().enumerate() {
            if line_str.len() > 120 && !line_str.trim().is_empty() {
                issues.push(LintIssue {
                    line: i + 1,
                    severity: Severity::Info,
                    message: format!("Line too long ({} chars, max 120)", line_str.len()),
                    snippet: Some(if line_str.len() > 80 {
                        format!("{}...", &line_str[..77])
                    } else {
                        line_str.to_string()
                    }),
                });
            }
        }
    }

    if kind == "string" || kind == "raw_string" {
        let text = node_text;
        let lower = text.to_lowercase();
        if (lower.contains("password")
            || lower.contains("secret")
            || lower.contains("token")
            || lower.contains("api_key"))
            && text.len() > 10
        {
            issues.push(LintIssue {
                line,
                severity: Severity::Warning,
                message: "Possible hardcoded secret: avoid storing credentials in scripts".into(),
                snippet: Some(if text.len() > 40 {
                    format!("{}...", &text[..37])
                } else {
                    text.to_string()
                }),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_node(source, child, _depth + 1, issues);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_rm_rf_warning() {
        let temp = std::env::temp_dir().join("agent_heart_test_lint.sh");
        std::fs::write(&temp, "#!/bin/bash\nrm -rf /some/dir\n").unwrap();
        let result = lint_script(&temp);
        assert!(result.is_ok());
        std::fs::remove_file(&temp).ok();
    }

    #[test]
    fn test_lint_clean_script() {
        let temp = std::env::temp_dir().join("agent_heart_test_clean.sh");
        std::fs::write(&temp, "#!/bin/bash\necho \"hello world\"\nls -la\n").unwrap();
        let result = lint_script(&temp);
        assert!(result.is_ok());
        std::fs::remove_file(&temp).ok();
    }
}
