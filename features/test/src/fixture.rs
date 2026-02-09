/// RAII fixtures for test isolation.
///
/// Provides `ScopedTempDir` for auto-cleaned temporary directories and
/// `ScopedFixture<T>` for generic resources with cleanup callbacks.

use std::path::{Path, PathBuf};

use crate::error::TestError;

// ── ScopedTempDir ────────────────────────────────────────────────────

/// RAII temporary directory that is automatically deleted on drop.
///
/// Wraps `tempfile::TempDir` with convenience helpers for creating
/// subdirectories and writing files within the temp directory.
///
/// # Example
///
/// ```
/// use swebash_test::fixture::ScopedTempDir;
///
/// let dir = ScopedTempDir::new("my_test").unwrap();
/// dir.write_file("config.yaml", "enabled: true").unwrap();
/// assert!(dir.path().join("config.yaml").exists());
/// // Directory is cleaned up when `dir` goes out of scope.
/// ```
pub struct ScopedTempDir {
    inner: tempfile::TempDir,
}

impl ScopedTempDir {
    /// Create a new temporary directory with the given prefix.
    pub fn new(prefix: &str) -> Result<Self, TestError> {
        let inner = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .map_err(|e| TestError::Fixture(format!("failed to create temp dir: {e}")))?;
        Ok(Self { inner })
    }

    /// Path to the temporary directory.
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Create a subdirectory within the temp directory.
    pub fn create_subdir(&self, name: &str) -> Result<PathBuf, TestError> {
        let path = self.inner.path().join(name);
        std::fs::create_dir_all(&path)
            .map_err(|e| TestError::Fixture(format!("failed to create subdir '{name}': {e}")))?;
        Ok(path)
    }

    /// Write a file within the temp directory.
    pub fn write_file(&self, relative_path: &str, content: &str) -> Result<PathBuf, TestError> {
        let path = self.inner.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TestError::Fixture(format!(
                    "failed to create parent dirs for '{relative_path}': {e}"
                ))
            })?;
        }
        std::fs::write(&path, content).map_err(|e| {
            TestError::Fixture(format!("failed to write file '{relative_path}': {e}"))
        })?;
        Ok(path)
    }
}

// ── ScopedEnvVar ────────────────────────────────────────────────────

/// RAII guard that sets an environment variable and restores the previous
/// value (or removes the variable) when dropped.
///
/// # Example
///
/// ```
/// use swebash_test::fixture::ScopedEnvVar;
///
/// {
///     let _guard = ScopedEnvVar::set("MY_TEST_VAR", "hello");
///     assert_eq!(std::env::var("MY_TEST_VAR").unwrap(), "hello");
/// }
/// // Variable restored to its previous state after drop.
/// ```
pub struct ScopedEnvVar {
    key: String,
    previous: Option<String>,
}

impl ScopedEnvVar {
    /// Set an environment variable, returning an RAII guard that restores
    /// the previous value on drop.
    pub fn set(key: &str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            previous,
        }
    }

    /// Remove an environment variable, returning an RAII guard that restores
    /// the previous value on drop.
    pub fn remove(key: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::remove_var(key);
        Self {
            key: key.to_string(),
            previous,
        }
    }

    /// The environment variable key managed by this guard.
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        match &self.previous {
            Some(val) => std::env::set_var(&self.key, val),
            None => std::env::remove_var(&self.key),
        }
    }
}

// ── ScopedFixture<T> ─────────────────────────────────────────────────

/// Generic RAII fixture that invokes a cleanup callback on drop.
///
/// Wraps any value `T` and ensures the cleanup function runs when the
/// fixture goes out of scope, regardless of test pass/fail.
///
/// # Example
///
/// ```
/// use swebash_test::fixture::ScopedFixture;
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use std::sync::Arc;
///
/// let cleaned = Arc::new(AtomicBool::new(false));
/// let cleaned_clone = cleaned.clone();
/// {
///     let _fixture = ScopedFixture::new(42, move |_val| {
///         cleaned_clone.store(true, Ordering::SeqCst);
///     });
/// }
/// assert!(cleaned.load(Ordering::SeqCst));
/// ```
pub struct ScopedFixture<T> {
    value: Option<T>,
    cleanup: Option<Box<dyn FnOnce(T) + Send>>,
}

impl<T> ScopedFixture<T> {
    /// Create a fixture wrapping `value` with a `cleanup` callback.
    pub fn new(value: T, cleanup: impl FnOnce(T) + Send + 'static) -> Self {
        Self {
            value: Some(value),
            cleanup: Some(Box::new(cleanup)),
        }
    }

    /// Access the wrapped value by reference.
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("fixture already dropped")
    }

    /// Access the wrapped value by mutable reference.
    pub fn get_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("fixture already dropped")
    }
}

impl<T> Drop for ScopedFixture<T> {
    fn drop(&mut self) {
        if let (Some(value), Some(cleanup)) = (self.value.take(), self.cleanup.take()) {
            cleanup(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn scoped_temp_dir_creates_directory() {
        let dir = ScopedTempDir::new("test_create").unwrap();
        assert!(dir.path().exists());
    }

    #[test]
    fn scoped_temp_dir_write_file() {
        let dir = ScopedTempDir::new("test_write").unwrap();
        let path = dir.write_file("hello.txt", "world").unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "world");
    }

    #[test]
    fn scoped_temp_dir_write_file_in_subdir() {
        let dir = ScopedTempDir::new("test_nested").unwrap();
        let path = dir.write_file("sub/dir/file.txt", "nested").unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "nested");
    }

    #[test]
    fn scoped_temp_dir_create_subdir() {
        let dir = ScopedTempDir::new("test_subdir").unwrap();
        let sub = dir.create_subdir("child").unwrap();
        assert!(sub.exists());
        assert!(sub.is_dir());
    }

    #[test]
    fn scoped_temp_dir_cleaned_on_drop() {
        let path;
        {
            let dir = ScopedTempDir::new("test_cleanup").unwrap();
            path = dir.path().to_path_buf();
            assert!(path.exists());
        }
        assert!(!path.exists(), "temp dir should be deleted on drop");
    }

    #[test]
    fn scoped_fixture_runs_cleanup_on_drop() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();
        {
            let _f = ScopedFixture::new(42, move |_| {
                cleaned_clone.store(true, Ordering::SeqCst);
            });
        }
        assert!(cleaned.load(Ordering::SeqCst));
    }

    #[test]
    fn scoped_fixture_get_returns_value() {
        let f = ScopedFixture::new("hello", |_| {});
        assert_eq!(*f.get(), "hello");
    }

    #[test]
    fn scoped_fixture_get_mut_allows_mutation() {
        let mut f = ScopedFixture::new(vec![1, 2], |_| {});
        f.get_mut().push(3);
        assert_eq!(f.get(), &vec![1, 2, 3]);
    }

    #[test]
    fn scoped_fixture_cleanup_receives_value() {
        let received = Arc::new(Mutex::new(None));
        let received_clone = received.clone();
        {
            let _f = ScopedFixture::new(99, move |val| {
                *received_clone.lock() = Some(val);
            });
        }
        assert_eq!(*received.lock(), Some(99));
    }

    use parking_lot::Mutex;

    // ── ScopedEnvVar tests ──────────────────────────────────────────
    // Note: env var tests use unique key names to avoid cross-test interference.

    #[test]
    fn scoped_env_var_sets_value() {
        let key = "SWEBASH_TEST_SET_1";
        std::env::remove_var(key); // ensure clean state
        let _guard = ScopedEnvVar::set(key, "hello");
        assert_eq!(std::env::var(key).unwrap(), "hello");
    }

    #[test]
    fn scoped_env_var_restores_on_drop() {
        let key = "SWEBASH_TEST_RESTORE_1";
        std::env::set_var(key, "original");
        {
            let _guard = ScopedEnvVar::set(key, "overridden");
            assert_eq!(std::env::var(key).unwrap(), "overridden");
        }
        assert_eq!(std::env::var(key).unwrap(), "original");
        std::env::remove_var(key);
    }

    #[test]
    fn scoped_env_var_removes_if_not_previously_set() {
        let key = "SWEBASH_TEST_REMOVE_AFTER_1";
        std::env::remove_var(key);
        {
            let _guard = ScopedEnvVar::set(key, "temp");
            assert_eq!(std::env::var(key).unwrap(), "temp");
        }
        assert!(std::env::var(key).is_err(), "should be removed after drop");
    }

    #[test]
    fn scoped_env_var_remove_clears_variable() {
        let key = "SWEBASH_TEST_CLEAR_1";
        std::env::set_var(key, "exists");
        {
            let _guard = ScopedEnvVar::remove(key);
            assert!(std::env::var(key).is_err());
        }
        assert_eq!(std::env::var(key).unwrap(), "exists");
        std::env::remove_var(key);
    }

    #[test]
    fn scoped_env_var_key_accessor() {
        let key = "SWEBASH_TEST_KEY_1";
        let guard = ScopedEnvVar::set(key, "val");
        assert_eq!(guard.key(), key);
    }

    #[test]
    fn scoped_env_var_remove_noop_when_unset() {
        let key = "SWEBASH_TEST_NOOP_1";
        std::env::remove_var(key);
        {
            let _guard = ScopedEnvVar::remove(key);
            assert!(std::env::var(key).is_err());
        }
        assert!(std::env::var(key).is_err());
    }
}
