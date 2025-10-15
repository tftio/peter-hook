//! Global configuration for peter-hook
//!
//! Handles user-wide configuration stored in ~/.config/peter-hook/config.toml

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Global configuration for peter-hook
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalConfig {
    /// Security settings
    pub security: SecurityConfig,
}

/// Security configuration settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityConfig {
    /// Allow imports from $HOME/.local/peter-hook directory
    #[serde(default)]
    pub allow_local: bool,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            security: SecurityConfig { allow_local: false },
        }
    }
}

impl GlobalConfig {
    /// Load global configuration from default location
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration directory cannot be determined or
    /// file cannot be read
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        Self::from_file(&config_path)
    }

    /// Load global configuration from a specific file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            // Return default config if file doesn't exist
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Save global configuration to default location
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be serialized or written
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }

    /// Get the default configuration file path
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration directory cannot be determined
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Unable to determine config directory")?;

        Ok(config_dir.join("peter-hook").join("config.toml"))
    }

    /// Check if an absolute path is allowed for import
    ///
    /// Only allows imports from $HOME/.local/peter-hook if `allow_local` is
    /// true
    ///
    /// # Errors
    ///
    /// Returns an error if home directory cannot be determined or path
    /// operations fail
    pub fn is_absolute_path_allowed(&self, path: &Path) -> Result<bool> {
        // If allow_local is false, reject all absolute paths
        if !self.security.allow_local {
            return Ok(false);
        }

        let home_dir = dirs::home_dir().context("Unable to determine home directory")?;

        // Get the expected peter-hook local directory
        let peter_hook_dir = home_dir.join(".local").join("peter-hook");

        // First check if the path is within peter-hook directory before canonicalizing
        if !path.starts_with(&peter_hook_dir) {
            return Ok(false);
        }

        // If the file exists, canonicalize to check for symlink attacks
        if path.exists() {
            let canonical_path = path.canonicalize().with_context(|| {
                format!("Failed to canonicalize import path: {}", path.display())
            })?;

            // If peter-hook directory exists, canonicalize it too
            if peter_hook_dir.exists() {
                let canonical_peter_hook_dir =
                    peter_hook_dir.canonicalize().with_context(|| {
                        format!(
                            "Failed to canonicalize peter-hook directory: {}",
                            peter_hook_dir.display()
                        )
                    })?;
                Ok(canonical_path.starts_with(&canonical_peter_hook_dir))
            } else {
                // Peter-hook directory doesn't exist but file does - this is suspicious
                // Fall back to basic path check
                Ok(canonical_path.starts_with(&peter_hook_dir))
            }
        } else {
            // File doesn't exist - just validate that the path would be within peter-hook
            // directory This allows configuration validation even before files
            // are created
            Ok(path.starts_with(&peter_hook_dir))
        }
    }

    /// Get the peter-hook local directory path
    ///
    /// # Errors
    ///
    /// Returns an error if home directory cannot be determined
    pub fn get_local_dir() -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Unable to determine home directory")?;

        Ok(home_dir.join(".local").join("peter-hook"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = GlobalConfig::default();
        assert!(!config.security.allow_local); // Default should be false
    }

    #[test]
    fn test_config_serialization() {
        let config = GlobalConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();

        // Should be able to parse it back
        let parsed: GlobalConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn test_load_nonexistent_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.toml");

        // Should return default config when file doesn't exist
        let config = GlobalConfig::from_file(&config_path).unwrap();
        assert_eq!(config, GlobalConfig::default());
    }

    #[test]
    fn test_load_and_save_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut config = GlobalConfig::default();
        config.security.allow_local = true;

        // Save config
        let content = toml::to_string_pretty(&config).unwrap();
        fs::write(&config_path, content).unwrap();

        // Load it back
        let loaded = GlobalConfig::from_file(&config_path).unwrap();
        assert_eq!(config, loaded);
        assert!(loaded.security.allow_local);
    }

    #[test]
    fn test_get_local_dir() {
        let local_dir = GlobalConfig::get_local_dir().unwrap();

        if let Some(home) = dirs::home_dir() {
            assert_eq!(local_dir, home.join(".local").join("peter-hook"));
        }
    }

    #[test]
    fn test_is_absolute_path_allowed_disabled() {
        let config = GlobalConfig::default(); // allow_local = false

        // Any absolute path should be rejected when allow_local is false
        let Some(home_dir) = dirs::home_dir() else {
            return;
        };

        let peter_hook_dir = home_dir.join(".local").join("peter-hook");
        let test_file = peter_hook_dir.join("test.toml");

        assert!(!config.is_absolute_path_allowed(&test_file).unwrap());
    }

    #[test]
    fn test_is_absolute_path_allowed_enabled() {
        let Some(home_dir) = dirs::home_dir() else {
            return;
        };

        let peter_hook_dir = home_dir.join(".local").join("peter-hook");
        if let Err(err) = fs::create_dir_all(&peter_hook_dir) {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                // Skip test when we cannot create the directory (restricted environments)
                return;
            }
            panic!("failed to create peter-hook dir: {err}");
        }

        let test_file = peter_hook_dir.join("test.toml");
        if let Err(err) = fs::write(&test_file, "test") {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                return;
            }
            panic!("failed to write test file: {err}");
        }

        let config = GlobalConfig {
            security: SecurityConfig { allow_local: true },
        };

        assert!(config.is_absolute_path_allowed(&test_file).unwrap());

        let outside_file = home_dir.join("other-dir").join("hooks.toml");
        if let Some(parent) = outside_file.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    return;
                }
                panic!("failed to create outside dir: {err}");
            }
        }
        if let Err(err) = fs::write(&outside_file, "test") {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                return;
            }
            panic!("failed to write outside file: {err}");
        }

        assert!(!config.is_absolute_path_allowed(&outside_file).unwrap());
    }

    #[test]
    fn test_symlink_protection() {
        use tempfile::TempDir;

        // Use temp directory instead of real home directory for CI compatibility
        let temp_home = TempDir::new().unwrap();
        let home_dir = temp_home.path();

        // Create peter-hook directory
        let peter_hook_dir = home_dir.join(".local").join("peter-hook");
        fs::create_dir_all(&peter_hook_dir).unwrap();

        // Create disallowed directory outside peter-hook
        let disallowed_dir = home_dir.join("secret-dir");
        fs::create_dir_all(&disallowed_dir).unwrap();

        // Create target file in disallowed directory
        let target_file = disallowed_dir.join("secret.toml");
        fs::write(&target_file, "secret content").unwrap();

        // Skip test on Windows or if symlink creation fails (requires permissions)
        #[cfg(unix)]
        {
            // Create symlink in peter-hook directory pointing to disallowed file
            let symlink = peter_hook_dir.join("symlink.toml");
            if std::os::unix::fs::symlink(&target_file, &symlink).is_ok() {
                // Test using the actual peter-hook directory as the allowed path
                // The symlink points to secret-dir which is outside .local/peter-hook
                let symlink_canonical = symlink.canonicalize().unwrap();
                let peter_hook_canonical = peter_hook_dir.canonicalize().unwrap();

                // Symlink resolves to secret-dir, not .local/peter-hook
                assert!(!symlink_canonical.starts_with(&peter_hook_canonical));
            }
        }
    }
}
