//! Global debug state management

use std::sync::atomic::{AtomicBool, Ordering};

/// Global debug state
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global trace state
static TRACE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable debug mode
pub fn enable() {
    DEBUG_ENABLED.store(true, Ordering::Relaxed);
}

/// Check if debug mode is enabled
pub fn is_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Enable trace mode
pub fn enable_trace() {
    TRACE_ENABLED.store(true, Ordering::Relaxed);
}

/// Check if trace mode is enabled
pub fn is_trace_enabled() -> bool {
    TRACE_ENABLED.load(Ordering::Relaxed)
}

/// Print trace message if trace mode is enabled
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        if $crate::debug::is_trace_enabled() {
            eprintln!("[TRACE] {}", format!($($arg)*));
        }
    };
}

/// Disable debug mode (for testing)
#[cfg(test)]
pub fn disable() {
    DEBUG_ENABLED.store(false, Ordering::Relaxed);
}

/// Disable trace mode (for testing)
#[cfg(test)]
pub fn disable_trace() {
    TRACE_ENABLED.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_initially_disabled() {
        // Reset state
        disable();
        assert!(!is_enabled(), "Debug should be disabled by default");
    }

    #[test]
    fn test_debug_enable() {
        disable();
        assert!(!is_enabled());

        enable();
        assert!(is_enabled(), "Debug should be enabled after enable()");

        // Clean up
        disable();
    }

    #[test]
    fn test_debug_enable_disable_toggle() {
        disable();
        assert!(!is_enabled());

        enable();
        assert!(is_enabled());

        disable();
        assert!(!is_enabled());

        enable();
        assert!(is_enabled());

        // Clean up
        disable();
    }

    #[test]
    fn test_debug_multiple_enables() {
        disable();

        enable();
        enable();
        enable();

        assert!(
            is_enabled(),
            "Should remain enabled after multiple enable() calls"
        );

        // Clean up
        disable();
    }
}
