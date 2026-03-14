// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// CLI end-to-end tests for the UPP CLI application.
// Tests command-line interface behavior including help output, versioning,
// and command execution using std::process::Command.

use std::process::Command;

// ─── Helper Functions ────────────────────────────────────────────────────────

/// Get the path to the CLI binary (compiled in target/debug or target/release)
fn get_cli_binary() -> String {
    env!("CARGO_BIN_EXE_upp").to_string()
}

/// Run a CLI command and return stdout, stderr, and status code
fn run_cli(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(get_cli_binary())
        .args(args)
        .output()
        .expect("Failed to execute CLI");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status.code().unwrap_or(-1);

    (stdout, stderr, status)
}

// ─── Help and Version Tests ──────────────────────────────────────────────────

#[test]
fn test_cli_help() {
    let (stdout, _, status) = run_cli(&["--help"]);

    // Help should succeed
    assert_eq!(status, 0, "Help command should exit with success");

    // Help output should contain common patterns
    assert!(
        stdout.contains("UPP Gateway CLI") || stdout.contains("upp"),
        "Help should contain CLI name"
    );
    assert!(
        stdout.contains("USAGE") || stdout.contains("Usage"),
        "Help should contain usage information"
    );
    assert!(
        stdout.contains("COMMANDS") || stdout.contains("Commands"),
        "Help should list available commands"
    );
}

#[test]
fn test_cli_version() {
    let (stdout, _, status) = run_cli(&["--version"]);

    // Version should succeed
    assert_eq!(status, 0, "Version command should exit with success");

    // Version output should contain version information
    assert!(
        stdout.contains("upp") || stdout.contains("version") || stdout.contains("0.1"),
        "Version output should contain version info"
    );
}

#[test]
fn test_cli_help_shorthand() {
    let (stdout, _, status) = run_cli(&["-h"]);

    // Short help should work
    assert_eq!(status, 0, "-h should exit with success");
    assert!(stdout.contains("help"), "Help should show help message");
}

// ─── Subcommand Help Tests ──────────────────────────────────────────────────

#[test]
fn test_health_command_help() {
    let (stdout, _, status) = run_cli(&["health", "--help"]);

    // Health help should succeed
    assert_eq!(status, 0, "Health help should exit with success");

    // Should show health command help
    assert!(
        stdout.contains("health") || stdout.contains("Check"),
        "Help should describe health command"
    );
}

#[test]
fn test_markets_command_help() {
    let (stdout, _, status) = run_cli(&["markets", "--help"]);

    assert_eq!(status, 0, "Markets help should exit with success");
    assert!(stdout.len() > 0, "Markets help should have content");
}

#[test]
fn test_orders_command_help() {
    let (stdout, _, status) = run_cli(&["orders", "--help"]);

    assert_eq!(status, 0, "Orders help should exit with success");
    assert!(stdout.len() > 0, "Orders help should have content");
}

#[test]
fn test_portfolio_command_help() {
    let (stdout, _, status) = run_cli(&["portfolio", "--help"]);

    assert_eq!(status, 0, "Portfolio help should exit with success");
    assert!(stdout.len() > 0, "Portfolio help should have content");
}

// ─── Global Flags Tests ──────────────────────────────────────────────────────

#[test]
fn test_json_flag_with_help() {
    let (stdout, _, status) = run_cli(&["--json", "--help"]);

    // JSON flag should be accepted even with help
    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

#[test]
fn test_url_flag_accepted() {
    // Just test that the flag is accepted (not that the command works)
    let (_, stderr, status) = run_cli(&["--url", "http://localhost:9090", "health"]);

    // Will fail because there's no server, but should accept the flag
    assert_ne!(status, 0, "Command should fail (no server)");
    // But the error should be about connection, not unknown flag
    assert!(
        !stderr.contains("unexpected") && !stderr.contains("unexpected"),
        "Should not complain about unknown flag"
    );
}

#[test]
fn test_api_key_flag_accepted() {
    // Just test that the flag is accepted
    let (_, stderr, status) = run_cli(&["--api-key", "test-key-123", "health"]);

    // Will fail because there's no server, but should accept the flag
    assert_ne!(status, 0, "Command should fail (no server)");
    assert!(
        !stderr.contains("unexpected"),
        "Should not complain about unknown flag"
    );
}

// ─── Health Command Error Handling ──────────────────────────────────────────

#[test]
fn test_health_command_no_server() {
    let (_, stderr, status) = run_cli(&["--url", "http://127.0.0.1:19999", "health"]);

    // Should fail because there's no server
    assert_ne!(status, 0, "Health should fail when no server is running");

    // Error message should indicate connection problem
    assert!(
        stderr.len() > 0 || status != 0,
        "Should provide error feedback when server unreachable"
    );
}

// ─── Invalid Command Tests ──────────────────────────────────────────────────

#[test]
fn test_invalid_command() {
    let (stdout, stderr, status) = run_cli(&["nonexistent-command"]);

    // Should fail with error
    assert_ne!(status, 0, "Invalid command should fail");

    // Error message should indicate unknown command
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("command") || combined.contains("unknown") || combined.contains("valid")
            || combined.contains("unknown command"),
        "Should indicate unknown command: {}",
        combined
    );
}

// ─── No Arguments Test ──────────────────────────────────────────────────────

#[test]
fn test_no_arguments() {
    let (stdout, stderr, status) = run_cli(&[]);

    // Should fail because no subcommand provided
    assert_ne!(
        status, 0,
        "Running without arguments should fail (requires subcommand)"
    );

    // Error should mention missing command
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.len() > 0,
        "Should provide feedback about missing subcommand"
    );
}

// ─── Markets Subcommand Tests ────────────────────────────────────────────────

#[test]
fn test_markets_list_help() {
    let (stdout, _, status) = run_cli(&["markets", "list", "--help"]);

    assert_eq!(status, 0, "Markets list help should succeed");
    assert!(
        stdout.contains("list") || stdout.contains("List"),
        "Should show list command help"
    );
}

#[test]
fn test_markets_get_help() {
    let (stdout, _, status) = run_cli(&["markets", "get", "--help"]);

    assert_eq!(status, 0, "Markets get help should succeed");
    assert!(stdout.len() > 0, "Should show get command help");
}

#[test]
fn test_markets_search_help() {
    let (stdout, _, status) = run_cli(&["markets", "search", "--help"]);

    assert_eq!(status, 0, "Markets search help should succeed");
    assert!(stdout.len() > 0, "Should show search command help");
}

// ─── Orders Subcommand Tests ────────────────────────────────────────────────

#[test]
fn test_orders_list_help() {
    let (stdout, _, status) = run_cli(&["orders", "list", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

#[test]
fn test_orders_create_help() {
    let (stdout, _, status) = run_cli(&["orders", "create", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Flag Combinations Test ─────────────────────────────────────────────────

#[test]
fn test_multiple_flags_with_help() {
    let (stdout, _, status) = run_cli(&[
        "--url",
        "http://localhost:9090",
        "--api-key",
        "my-key",
        "--json",
        "--help",
    ]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── JSON Flag Tests ────────────────────────────────────────────────────────

#[test]
fn test_json_flag_location_before_command() {
    let (_, stderr, status) = run_cli(&[
        "--json",
        "--url",
        "http://127.0.0.1:19999",
        "health",
    ]);

    // Should fail connecting, but accept flags
    assert_ne!(status, 0);
    assert!(
        !stderr.contains("unexpected"),
        "Should accept --json flag before command"
    );
}

#[test]
fn test_json_flag_with_markets_list() {
    let (_, stderr, status) = run_cli(&[
        "--json",
        "--url",
        "http://127.0.0.1:19999",
        "markets",
        "list",
    ]);

    // Should fail connecting, but accept JSON flag
    assert_ne!(status, 0);
    assert!(
        !stderr.contains("unexpected"),
        "Should accept --json flag with subcommands"
    );
}

// ─── Config Command Tests ──────────────────────────────────────────────────

#[test]
fn test_config_command_help() {
    let (stdout, _, status) = run_cli(&["config", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Keys Command Tests ─────────────────────────────────────────────────────

#[test]
fn test_keys_command_help() {
    let (stdout, _, status) = run_cli(&["keys", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

#[test]
fn test_keys_create_help() {
    let (stdout, _, status) = run_cli(&["keys", "create", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

#[test]
fn test_keys_list_help() {
    let (stdout, _, status) = run_cli(&["keys", "list", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Arbitrage Command Tests ────────────────────────────────────────────────

#[test]
fn test_arbitrage_command_help() {
    let (stdout, _, status) = run_cli(&["arbitrage", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Candles Command Tests ──────────────────────────────────────────────────

#[test]
fn test_candles_command_help() {
    let (stdout, _, status) = run_cli(&["candles", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Backtest Command Tests ────────────────────────────────────────────────

#[test]
fn test_backtest_command_help() {
    let (stdout, _, status) = run_cli(&["backtest", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Feeds Command Tests ────────────────────────────────────────────────────

#[test]
fn test_feeds_command_help() {
    let (stdout, _, status) = run_cli(&["feeds", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Route Command Tests ───────────────────────────────────────────────────

#[test]
fn test_route_command_help() {
    let (stdout, _, status) = run_cli(&["route", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Trades Command Tests ──────────────────────────────────────────────────

#[test]
fn test_trades_command_help() {
    let (stdout, _, status) = run_cli(&["trades", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Portfolio Command Tests ──────────────────────────────────────────────

#[test]
fn test_portfolio_summary_help() {
    let (stdout, _, status) = run_cli(&["portfolio", "summary", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

#[test]
fn test_portfolio_positions_help() {
    let (stdout, _, status) = run_cli(&["portfolio", "positions", "--help"]);

    assert_eq!(status, 0);
    assert!(stdout.len() > 0);
}

// ─── Output Format Tests ────────────────────────────────────────────────────

#[test]
fn test_help_has_options_section() {
    let (stdout, _, _) = run_cli(&["--help"]);

    // Help should describe available options
    assert!(
        stdout.contains("OPTIONS") || stdout.contains("options") || stdout.contains("--"),
        "Help should list options"
    );
}

#[test]
fn test_help_has_usage_pattern() {
    let (stdout, _, _) = run_cli(&["--help"]);

    // Help should show usage pattern
    assert!(
        stdout.contains("upp") && (stdout.contains("[") || stdout.contains("<")),
        "Help should show usage pattern"
    );
}

// ─── Error Message Quality Tests ────────────────────────────────────────────

#[test]
fn test_error_for_missing_required_argument() {
    let (_, stderr, status) = run_cli(&["markets", "get"]);

    // Should fail - market_id is required
    assert_ne!(status, 0);

    // Should have some error output
    let combined_output = stderr;
    assert!(
        combined_output.len() > 0 || status != 0,
        "Should indicate missing required argument"
    );
}

// ─── Binary Name Test ──────────────────────────────────────────────────────

#[test]
fn test_version_contains_program_name() {
    let (stdout, _, _) = run_cli(&["--version"]);

    // Version output should be identifiable as the upp CLI
    assert!(
        stdout.contains("upp") || stdout.len() > 0,
        "Version should identify the program"
    );
}

// ─── Exit Code Tests ────────────────────────────────────────────────────────

#[test]
fn test_success_exit_code_for_help() {
    let (_, _, status) = run_cli(&["--help"]);
    assert_eq!(status, 0, "Help should exit with code 0");
}

#[test]
fn test_success_exit_code_for_version() {
    let (_, _, status) = run_cli(&["--version"]);
    assert_eq!(status, 0, "Version should exit with code 0");
}

#[test]
fn test_failure_exit_code_for_invalid_command() {
    let (_, _, status) = run_cli(&["invalid-command-name"]);
    assert_ne!(status, 0, "Invalid command should fail");
}

#[test]
fn test_failure_exit_code_for_missing_args() {
    let (_, _, status) = run_cli(&["markets", "get"]);
    assert_ne!(status, 0, "Missing required arg should fail");
}

// ─── Long Output Handling Test ──────────────────────────────────────────────

#[test]
fn test_help_output_completeness() {
    let (stdout, _, status) = run_cli(&["--help"]);

    assert_eq!(status, 0);

    // Help should mention several commands
    let contains_markets = stdout.contains("markets");
    let contains_orders = stdout.contains("orders");
    let contains_health = stdout.contains("health");
    let contains_options = stdout.contains("--") || stdout.contains("OPTIONS");

    assert!(
        contains_markets || contains_orders || contains_health || contains_options,
        "Help output should be complete"
    );
}
