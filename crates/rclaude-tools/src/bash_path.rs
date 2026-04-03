//! Bash path validation for command arguments.
//!
//! Extracts file paths from command arguments and validates them against
//! the working directory to prevent unauthorized file access.

use std::path::{Path, PathBuf};

/// File operation type for a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOpType {
    Read,
    Write,
    Create,
}

/// Result of path validation.
#[derive(Debug, Clone)]
pub struct PathValidationResult {
    pub allowed: bool,
    pub reason: String,
    pub paths: Vec<PathBuf>,
}

/// Dangerous removal paths that always require explicit approval.
const DANGEROUS_PATHS: &[&str] = &[
    "/", "/home", "/etc", "/usr", "/var", "/boot", "/sys", "/proc", "/bin", "/sbin", "/lib",
    "/lib64", "/opt", "/root", "/tmp",
];

/// Check if a path is a dangerous removal target.
pub fn is_dangerous_removal_path(path: &str) -> bool {
    let normalized = if path == "/" {
        "/"
    } else {
        path.trim_end_matches('/')
    };
    DANGEROUS_PATHS.contains(&normalized)
}

/// Expand ~ to home directory.
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

/// Filter out flags (args starting with -) from argument list.
/// Respects -- end-of-options delimiter.
fn filter_out_flags(args: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut after_double_dash = false;
    let mut skip_next = false;

    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if !after_double_dash && arg == "--" {
            after_double_dash = true;
            continue;
        }
        if !after_double_dash && arg.starts_with('-') {
            continue;
        }
        result.push(arg.clone());
    }
    result
}

/// Extract paths from a pattern-based command (grep, rg).
/// First non-flag arg is the pattern, rest are paths.
fn parse_pattern_command(args: &[String], flag_args: &[&str], default: &[&str]) -> Vec<String> {
    let mut paths = Vec::new();
    let mut pattern_found = false;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with('-') {
            if flag_args.contains(&arg.as_str()) {
                i += 1;
            } // skip flag value
            i += 1;
            continue;
        }
        if !pattern_found {
            pattern_found = true;
            i += 1;
            continue;
        }
        paths.push(arg.clone());
        i += 1;
    }

    if paths.is_empty() {
        default.iter().map(|s| s.to_string()).collect()
    } else {
        paths
    }
}

/// Command operation types (matching COMMAND_OPERATION_TYPE).
pub fn command_op_type(cmd: &str) -> Option<FileOpType> {
    match cmd {
        "cd" | "ls" | "find" | "cat" | "head" | "tail" | "sort" | "uniq" | "wc" | "cut"
        | "paste" | "column" | "tr" | "file" | "stat" | "diff" | "awk" | "strings" | "hexdump"
        | "od" | "base64" | "nl" | "grep" | "rg" | "git" | "jq" | "sha256sum" | "sha1sum"
        | "md5sum" => Some(FileOpType::Read),
        "mkdir" | "touch" => Some(FileOpType::Create),
        "rm" | "rmdir" | "mv" | "cp" | "sed" => Some(FileOpType::Write),
        _ => None,
    }
}

/// Extract file paths from command arguments (matching PATH_EXTRACTORS).
pub fn extract_paths(cmd: &str, args: &[String]) -> Vec<String> {
    match cmd {
        "cd" => {
            if args.is_empty() {
                vec![dirs::home_dir()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".into())]
            } else {
                vec![args.join(" ")]
            }
        }
        "ls" => {
            let paths = filter_out_flags(args);
            if paths.is_empty() {
                vec![".".into()]
            } else {
                paths
            }
        }
        "find" => {
            let mut paths = Vec::new();
            let mut found_flag = false;
            let path_flags = [
                "-newer",
                "-anewer",
                "-cnewer",
                "-samefile",
                "-path",
                "-wholename",
            ];

            for (i, arg) in args.iter().enumerate() {
                if arg == "--" {
                    continue;
                }
                if arg.starts_with('-') {
                    if ["-H", "-L", "-P"].contains(&arg.as_str()) {
                        continue;
                    }
                    found_flag = true;
                    if path_flags.contains(&arg.as_str()) {
                        if let Some(next) = args.get(i + 1) {
                            paths.push(next.clone());
                        }
                    }
                    continue;
                }
                if !found_flag {
                    paths.push(arg.clone());
                }
            }
            if paths.is_empty() {
                vec![".".into()]
            } else {
                paths
            }
        }
        "grep" => {
            let flags = [
                "-e",
                "--regexp",
                "-f",
                "--file",
                "--exclude",
                "--include",
                "--exclude-dir",
                "-m",
                "--max-count",
                "-A",
                "-B",
                "-C",
            ];
            parse_pattern_command(args, &flags, &[])
        }
        "rg" => {
            let flags = [
                "-e",
                "--regexp",
                "-f",
                "--file",
                "-t",
                "--type",
                "-g",
                "--glob",
                "-m",
                "--max-count",
                "-A",
                "-B",
                "-C",
            ];
            parse_pattern_command(args, &flags, &["."])
        }
        "sed" => {
            let mut paths = Vec::new();
            let mut script_found = false;
            let mut skip_next = false;
            for (i, arg) in args.iter().enumerate() {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if arg.starts_with('-') {
                    if ["-f", "--file"].contains(&arg.as_str()) {
                        if let Some(next) = args.get(i + 1) {
                            paths.push(next.clone());
                        }
                        skip_next = true;
                        script_found = true;
                    } else if ["-e", "--expression"].contains(&arg.as_str()) {
                        skip_next = true;
                        script_found = true;
                    }
                    continue;
                }
                if !script_found {
                    script_found = true;
                    continue;
                }
                paths.push(arg.clone());
            }
            paths
        }
        "git" => {
            // Only git diff --no-index needs path validation
            if args.first().is_some_and(|a| a == "diff") && args.contains(&"--no-index".to_string())
            {
                let file_paths = filter_out_flags(&args[1..]);
                file_paths.into_iter().take(2).collect()
            } else {
                vec![]
            }
        }
        "jq" => {
            let mut paths = Vec::new();
            let flag_args = [
                "-e",
                "-f",
                "--from-file",
                "--arg",
                "--argjson",
                "--slurpfile",
                "-L",
            ];
            let mut filter_found = false;
            let mut i = 0;
            while i < args.len() {
                let arg = &args[i];
                if arg.starts_with('-') {
                    if flag_args.contains(&arg.as_str()) {
                        i += 1;
                    }
                    i += 1;
                    continue;
                }
                if !filter_found {
                    filter_found = true;
                    i += 1;
                    continue;
                }
                paths.push(arg.clone());
                i += 1;
            }
            paths
        }
        // Simple commands: just filter out flags
        _ => filter_out_flags(args),
    }
}

/// Validate paths extracted from a command against the working directory.
pub fn validate_command_paths(cmd: &str, args: &[String], cwd: &Path) -> PathValidationResult {
    let paths = extract_paths(cmd, args);
    let op_type = command_op_type(cmd).unwrap_or(FileOpType::Read);

    let mut resolved_paths = Vec::new();
    for path_str in &paths {
        let clean = path_str.trim_matches(|c| c == '\'' || c == '"');
        let expanded = expand_tilde(clean);
        let resolved = if Path::new(&expanded).is_absolute() {
            PathBuf::from(&expanded)
        } else {
            cwd.join(&expanded)
        };
        // Normalize
        let normalized = rclaude_utils::path::normalize(&resolved);
        resolved_paths.push(normalized);
    }

    // Check for dangerous removal paths
    if cmd == "rm" || cmd == "rmdir" {
        for path in &resolved_paths {
            let path_str = path.to_string_lossy();
            if is_dangerous_removal_path(&path_str) {
                return PathValidationResult {
                    allowed: false,
                    reason: format!("Dangerous removal target: {}", path_str),
                    paths: resolved_paths,
                };
            }
        }
    }

    // For write/create operations, check paths are within cwd or home
    if op_type == FileOpType::Write || op_type == FileOpType::Create {
        let cwd_str = cwd.to_string_lossy();
        let home = dirs::home_dir().unwrap_or_default();
        for path in &resolved_paths {
            let path_str = path.to_string_lossy();
            if !path_str.starts_with(cwd_str.as_ref())
                && !path_str.starts_with(home.to_string_lossy().as_ref())
                && !path_str.starts_with("/tmp")
            {
                return PathValidationResult {
                    allowed: false,
                    reason: format!("Write to path outside project/home: {}", path_str),
                    paths: resolved_paths,
                };
            }
        }
    }

    PathValidationResult {
        allowed: true,
        reason: "All paths validated".into(),
        paths: resolved_paths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_paths_ls() {
        let paths = extract_paths("ls", &["-la".into(), "/tmp".into()]);
        assert_eq!(paths, vec!["/tmp"]);
    }

    #[test]
    fn test_extract_paths_ls_default() {
        let paths = extract_paths("ls", &["-la".into()]);
        assert_eq!(paths, vec!["."]);
    }

    #[test]
    fn test_extract_paths_find() {
        let paths = extract_paths("find", &[".".into(), "-name".into(), "*.rs".into()]);
        assert_eq!(paths, vec!["."]);
    }

    #[test]
    fn test_extract_paths_grep() {
        let paths = extract_paths(
            "grep",
            &["pattern".into(), "file1.txt".into(), "file2.txt".into()],
        );
        assert_eq!(paths, vec!["file1.txt", "file2.txt"]);
    }

    #[test]
    fn test_extract_paths_sed() {
        let paths = extract_paths("sed", &["-i".into(), "s/a/b/".into(), "file.txt".into()]);
        assert_eq!(paths, vec!["file.txt"]);
    }

    #[test]
    fn test_extract_paths_git_diff_no_index() {
        let paths = extract_paths(
            "git",
            &[
                "diff".into(),
                "--no-index".into(),
                "a.txt".into(),
                "b.txt".into(),
            ],
        );
        assert_eq!(paths, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn test_extract_paths_git_status() {
        let paths = extract_paths("git", &["status".into()]);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_dangerous_removal() {
        assert!(is_dangerous_removal_path("/"));
        assert!(is_dangerous_removal_path("/home"));
        assert!(is_dangerous_removal_path("/etc/"));
        assert!(!is_dangerous_removal_path("/home/user/project"));
    }

    #[test]
    fn test_validate_rm_dangerous() {
        let result =
            validate_command_paths("rm", &["-rf".into(), "/".into()], Path::new("/home/user"));
        assert!(!result.allowed);
        assert!(result.reason.contains("Dangerous"));
    }

    #[test]
    fn test_validate_write_outside_cwd() {
        let result = validate_command_paths(
            "cp",
            &["file.txt".into(), "/etc/passwd".into()],
            Path::new("/home/user/project"),
        );
        assert!(!result.allowed);
        assert!(result.reason.contains("outside"));
    }

    #[test]
    fn test_validate_read_anywhere() {
        let result = validate_command_paths("cat", &["/etc/hosts".into()], Path::new("/home/user"));
        assert!(result.allowed);
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test");
        assert!(!expanded.starts_with('~'));
    }

    #[test]
    fn test_command_op_types() {
        assert_eq!(command_op_type("cat"), Some(FileOpType::Read));
        assert_eq!(command_op_type("rm"), Some(FileOpType::Write));
        assert_eq!(command_op_type("mkdir"), Some(FileOpType::Create));
        assert_eq!(command_op_type("unknown"), None);
    }
}
