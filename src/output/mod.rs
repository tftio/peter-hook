//! Output formatting utilities

use console::{Emoji, style};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;

/// Output formatter that strips colors and emojis for non-TTY output
pub struct OutputFormatter {
    /// Whether output is going to a TTY
    is_tty: bool,
}

impl OutputFormatter {
    /// Create a new output formatter
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_tty: std::io::stdout().is_terminal(),
        }
    }

    /// Create a formatter with explicit TTY setting (for testing)
    #[must_use]
    pub const fn with_tty(is_tty: bool) -> Self {
        Self { is_tty }
    }

    /// Format a status symbol (check mark, X, etc.)
    #[must_use]
    pub fn status(&self, success: bool) -> String {
        if self.is_tty {
            if success {
                format!("{}", style("✓").green().bold())
            } else {
                format!("{}", style("✗").red().bold())
            }
        } else if success {
            "[PASS]".to_string()
        } else {
            "[FAIL]".to_string()
        }
    }

    /// Create a progress bar for hook execution
    ///
    /// # Panics
    ///
    /// Panics if the progress bar template is invalid
    #[must_use]
    pub fn create_progress_bar(&self, total: u64) -> Option<ProgressBar> {
        if self.is_tty && total > 1 {
            let pb = ProgressBar::new(total);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                    .unwrap()
                    .progress_chars("🚀🌟✨"),
            );
            Some(pb)
        } else {
            None
        }
    }

    /// Format hook execution start
    #[must_use]
    pub fn hook_start(&self, name: &str) -> String {
        if self.is_tty {
            format!("{} {}", Emoji("🔧", ""), style(name).cyan().bold())
        } else {
            format!("Running: {name}")
        }
    }

    /// Format hook execution result
    #[must_use]
    pub fn hook_result(&self, name: &str, success: bool, exit_code: i32) -> String {
        if self.is_tty {
            let status = if success {
                format!("{}", style("✓").green().bold())
            } else {
                format!("{}", style("✗").red().bold())
            };
            let name_styled = if success {
                style(name).green()
            } else {
                style(name).red()
            };
            format!("{status} {name_styled}: exit code {exit_code}")
        } else {
            let status = if success { "[PASS]" } else { "[FAIL]" };
            format!("{status} {name}: exit code {exit_code}")
        }
    }

    /// Format section header
    #[must_use]
    pub fn section_header(&self, title: &str) -> String {
        if self.is_tty {
            format!(
                "\n{} {}",
                Emoji("📊", "==="),
                style(title).bold().underlined()
            )
        } else {
            format!("=== {title} ===")
        }
    }

    /// Format overall result with style
    #[must_use]
    pub fn overall_result(&self, success: bool) -> String {
        if self.is_tty {
            if success {
                format!(
                    "\n{} {}",
                    Emoji("🎉", "[SUCCESS]"),
                    style("All hooks completed successfully!").green().bold()
                )
            } else {
                format!(
                    "\n{} {}",
                    Emoji("💥", "[FAILURE]"),
                    style("Some hooks failed").red().bold()
                )
            }
        } else {
            let status = if success { "SUCCESS" } else { "FAILURE" };
            format!("Overall: {status}")
        }
    }

    /// Format a managed/custom status
    #[must_use]
    pub const fn managed_status(&self, is_managed: bool) -> &'static str {
        if self.is_tty {
            if is_managed {
                "🔧 managed"
            } else {
                "📄 custom"
            }
        } else if is_managed {
            "[managed]"
        } else {
            "[custom]"
        }
    }

    /// Format a restore symbol
    #[must_use]
    pub const fn restore(&self) -> &'static str {
        if self.is_tty { "🔄" } else { "[RESTORE]" }
    }

    /// Format a backup symbol
    #[must_use]
    pub const fn backup(&self) -> &'static str {
        if self.is_tty { "💾" } else { "[BACKUP]" }
    }

    /// Format a skip symbol
    #[must_use]
    pub const fn skip(&self) -> &'static str {
        if self.is_tty { "⏭️" } else { "[SKIP]" }
    }

    /// Format section divider  
    #[must_use]
    pub fn divider(&self, title: &str) -> String {
        if self.is_tty {
            format!("{}\n{}", title, "=".repeat(title.len()))
        } else {
            format!("=== {title} ===")
        }
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Global output formatter instance
static OUTPUT_FORMATTER: std::sync::LazyLock<OutputFormatter> =
    std::sync::LazyLock::new(OutputFormatter::new);

/// Get the global output formatter
#[must_use]
pub fn formatter() -> &'static OutputFormatter {
    &OUTPUT_FORMATTER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatter_creation() {
        let formatter = OutputFormatter::new();
        // Can't easily test TTY detection in unit tests, but ensure it doesn't panic
        let _ = formatter.status(true);
        let _ = formatter.status(false);
    }

    #[test]
    fn test_non_tty_output() {
        let formatter = OutputFormatter { is_tty: false };

        assert_eq!(formatter.status(true), "[PASS]");
        assert_eq!(formatter.status(false), "[FAIL]");
        assert_eq!(formatter.managed_status(true), "[managed]");
        assert_eq!(formatter.managed_status(false), "[custom]");
    }

    #[test]
    fn test_tty_output() {
        let formatter = OutputFormatter { is_tty: true };

        assert_eq!(console::strip_ansi_codes(&formatter.status(true)), "✓");
        assert_eq!(console::strip_ansi_codes(&formatter.status(false)), "✗");
        assert_eq!(formatter.managed_status(true), "🔧 managed");
        assert_eq!(formatter.managed_status(false), "📄 custom");
    }

    #[test]
    fn test_divider_formatting() {
        let formatter_tty = OutputFormatter { is_tty: true };
        let formatter_no_tty = OutputFormatter { is_tty: false };

        assert_eq!(formatter_tty.divider("Test"), "Test\n====");
        assert_eq!(formatter_no_tty.divider("Test"), "=== Test ===");
    }
}
