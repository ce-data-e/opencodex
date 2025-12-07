//! Security deny list checking for commands, applied regardless of approval policy.
//!
//! This module provides functionality to check commands against configured deny
//! and forbidden patterns. These checks apply even when `--yolo` mode is active,
//! providing a safety net for dangerous operations.

use regex::Regex;

use crate::bash::parse_shell_lc_plain_commands;
use crate::config::types::SecurityPolicy;

/// Result of checking a command against the security deny list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyListCheckResult {
    /// Command is allowed to proceed to normal approval flow.
    Allowed,
    /// Command requires approval even in YOLO mode.
    RequiresApproval { matched_pattern: String },
    /// Command is forbidden and should be rejected.
    Forbidden { matched_pattern: String },
}

/// Check a command against the security deny list.
///
/// This function parses shell-wrapped commands (e.g., `bash -lc "cmd1 && cmd2"`)
/// and checks each inner command against the deny and forbidden patterns.
///
/// # Arguments
/// * `command` - The command as a vector of strings (program and arguments)
/// * `security_policy` - The security policy containing deny and forbidden patterns
///
/// # Returns
/// * `DenyListCheckResult::Forbidden` - If any inner command matches a forbidden pattern
/// * `DenyListCheckResult::RequiresApproval` - If any inner command matches a deny pattern
/// * `DenyListCheckResult::Allowed` - If no patterns match
pub fn check_command_against_deny_list(
    command: &[String],
    security_policy: &SecurityPolicy,
) -> DenyListCheckResult {
    // Parse inner commands from shell wrappers like `bash -lc "cmd1 && cmd2"`
    let commands =
        parse_shell_lc_plain_commands(command).unwrap_or_else(|| vec![command.to_vec()]);

    for cmd in &commands {
        let command_str = cmd.join(" ");

        // Check forbidden patterns first (higher priority)
        if let Some(matched) =
            find_matching_pattern(&command_str, &security_policy.forbidden_commands)
        {
            return DenyListCheckResult::Forbidden {
                matched_pattern: matched,
            };
        }

        // Check deny (force approval) patterns
        if let Some(matched) = find_matching_pattern(&command_str, &security_policy.deny_commands) {
            return DenyListCheckResult::RequiresApproval {
                matched_pattern: matched,
            };
        }
    }

    DenyListCheckResult::Allowed
}

/// Find the first pattern that matches the command string.
fn find_matching_pattern(command_str: &str, patterns: &[Regex]) -> Option<String> {
    patterns
        .iter()
        .find(|pattern| pattern.is_match(command_str))
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::SecurityPolicyToml;
    use pretty_assertions::assert_eq;

    fn make_policy(deny: Vec<&str>, forbidden: Vec<&str>) -> SecurityPolicy {
        SecurityPolicyToml {
            deny_commands: Some(deny.into_iter().map(String::from).collect()),
            forbidden_commands: Some(forbidden.into_iter().map(String::from).collect()),
        }
        .into()
    }

    #[test]
    fn test_allowed_command_passes() {
        let policy = make_policy(vec![r"rm\s+-rf"], vec![]);
        let result =
            check_command_against_deny_list(&["ls".into(), "-la".into()], &policy);
        assert_eq!(result, DenyListCheckResult::Allowed);
    }

    #[test]
    fn test_forbidden_command_detected() {
        let policy = make_policy(vec![], vec![r"rm\s+-rf\s+/"]);
        let result = check_command_against_deny_list(
            &["rm".into(), "-rf".into(), "/".into()],
            &policy,
        );
        assert!(matches!(result, DenyListCheckResult::Forbidden { .. }));
        if let DenyListCheckResult::Forbidden { matched_pattern } = result {
            assert_eq!(matched_pattern, r"rm\s+-rf\s+/");
        }
    }

    #[test]
    fn test_deny_command_requires_approval() {
        let policy = make_policy(vec![r"git\s+push\s+--force"], vec![]);
        let result = check_command_against_deny_list(
            &["git".into(), "push".into(), "--force".into()],
            &policy,
        );
        assert!(matches!(result, DenyListCheckResult::RequiresApproval { .. }));
    }

    #[test]
    fn test_forbidden_takes_precedence_over_deny() {
        // If a command matches both forbidden and deny, forbidden wins
        let policy = make_policy(vec![r"rm"], vec![r"rm"]);
        let result = check_command_against_deny_list(&["rm".into(), "file.txt".into()], &policy);
        assert!(matches!(result, DenyListCheckResult::Forbidden { .. }));
    }

    #[test]
    fn test_bash_wrapped_command_detected() {
        let policy = make_policy(vec![r"git\s+reset\s+--hard"], vec![]);
        let result = check_command_against_deny_list(
            &[
                "bash".into(),
                "-lc".into(),
                "git reset --hard HEAD~1".into(),
            ],
            &policy,
        );
        assert!(matches!(result, DenyListCheckResult::RequiresApproval { .. }));
    }

    #[test]
    fn test_multi_command_script_any_match() {
        // If any command in a multi-command script matches, it should be caught
        let policy = make_policy(vec![r"git\s+push\s+--force"], vec![]);
        let result = check_command_against_deny_list(
            &[
                "bash".into(),
                "-lc".into(),
                "echo hello && git push --force && echo done".into(),
            ],
            &policy,
        );
        assert!(matches!(result, DenyListCheckResult::RequiresApproval { .. }));
    }

    #[test]
    fn test_empty_policy_allows_all() {
        let policy = SecurityPolicy::default();
        let result = check_command_against_deny_list(
            &["rm".into(), "-rf".into(), "/".into()],
            &policy,
        );
        assert_eq!(result, DenyListCheckResult::Allowed);
    }

    #[test]
    fn test_regex_pattern_matching() {
        // Test that regex patterns work correctly
        let policy = make_policy(vec![r"^git\s+(push|reset)"], vec![]);

        // Should match
        let result = check_command_against_deny_list(&["git".into(), "push".into()], &policy);
        assert!(matches!(result, DenyListCheckResult::RequiresApproval { .. }));

        let result = check_command_against_deny_list(&["git".into(), "reset".into()], &policy);
        assert!(matches!(result, DenyListCheckResult::RequiresApproval { .. }));

        // Should not match
        let result = check_command_against_deny_list(&["git".into(), "status".into()], &policy);
        assert_eq!(result, DenyListCheckResult::Allowed);
    }

    #[test]
    fn test_invalid_regex_patterns_are_skipped() {
        // Invalid patterns should be filtered out during SecurityPolicy::from()
        let policy_toml = SecurityPolicyToml {
            deny_commands: Some(vec![
                r"[invalid".to_string(), // Invalid regex
                r"valid".to_string(),    // Valid regex
            ]),
            forbidden_commands: None,
        };
        let policy: SecurityPolicy = policy_toml.into();

        // Only the valid pattern should be present
        assert_eq!(policy.deny_commands.len(), 1);
        assert_eq!(policy.deny_commands[0].as_str(), "valid");
    }
}
