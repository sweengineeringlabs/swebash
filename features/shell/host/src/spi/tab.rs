use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use wasmtime::*;

use super::state::{HostState, SandboxPolicy};

/// Unique identifier for a tab.
pub type TabId = u32;

/// Lightweight discriminant for tab type. `Copy + Eq` so it can be matched
/// after borrowing the tab without holding a borrow on `TabInner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Shell,
    Ai,
    HistoryView,
}

/// A WASM engine instance and its associated Wasmtime store.
#[allow(dead_code)]
pub struct WasmSession {
    pub store: Store<HostState>,
    pub instance: Instance,
    pub buf_ptr: usize,
    pub buf_cap: usize,
    pub shell_eval: TypedFunc<u32, ()>,
    pub memory: Memory,
}

/// Type-level encoding of tab content. The WASM session is embedded directly
/// in the `Shell` variant, guaranteeing at compile time that shell tabs always
/// have an engine and mode tabs never do.
pub enum TabInner {
    /// A regular shell session — always backed by a WASM engine.
    Shell(WasmSession),
    /// A dedicated AI chat tab.
    Ai { fallback_cwd: PathBuf },
    /// A scrollable history browser.
    HistoryView { fallback_cwd: PathBuf },
}

/// A single tab in the shell.
pub struct Tab {
    pub id: TabId,
    pub inner: TabInner,
    pub label: String,
    /// Per-tab multiline input buffer.
    pub multiline_buffer: String,
    /// Per-tab recent commands for AI context.
    pub recent_commands: Vec<String>,
    /// Whether this tab is in AI mode (shell tabs only).
    pub ai_mode: bool,
    /// Active AI agent id for the prompt.
    pub ai_agent_id: String,
}

impl Tab {
    /// Return the lightweight discriminant for this tab's type.
    pub fn kind(&self) -> TabKind {
        match &self.inner {
            TabInner::Shell(_) => TabKind::Shell,
            TabInner::Ai { .. } => TabKind::Ai,
            TabInner::HistoryView { .. } => TabKind::HistoryView,
        }
    }

    /// Read the virtual CWD for this tab. For shell tabs it comes from the
    /// WASM store's `HostState`; for mode tabs from a local fallback field.
    pub fn virtual_cwd(&self) -> PathBuf {
        match &self.inner {
            TabInner::Shell(wasm) => wasm.store.data().virtual_cwd.clone(),
            TabInner::Ai { fallback_cwd, .. } => fallback_cwd.clone(),
            TabInner::HistoryView { fallback_cwd } => fallback_cwd.clone(),
        }
    }

    /// Build a short display label for the tab bar: "[1:>:~/proj]"
    pub fn display_label(&self, home: Option<&PathBuf>) -> String {
        let icon = match self.kind() {
            TabKind::Shell => ">",
            TabKind::Ai => "AI",
            TabKind::HistoryView => "H",
        };
        if !self.label.is_empty() {
            return format!("{}:{}", icon, self.label);
        }
        let cwd = self.virtual_cwd();
        let cwd_str = cwd.to_string_lossy();
        let short = match home {
            Some(h) => {
                let home_str = h.to_string_lossy();
                if cwd_str == home_str.as_ref() {
                    "~".to_string()
                } else if let Some(rest) = cwd_str.strip_prefix(home_str.as_ref()) {
                    if rest.starts_with('/') || rest.starts_with('\\') {
                        format!("~{rest}")
                    } else {
                        cwd_str.into_owned()
                    }
                } else {
                    cwd_str.into_owned()
                }
            }
            None => cwd_str.into_owned(),
        };
        format!("{icon}:{short}")
    }
}

/// Manages all open tabs and tracks which one is active.
pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active: usize,
    next_id: TabId,
    /// Shared history reference across all tabs.
    pub history: Arc<Mutex<swebash_readline::History>>,
}

impl TabManager {
    /// Create a new tab manager with a shared history.
    pub fn new(history: Arc<Mutex<swebash_readline::History>>) -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            history,
        }
    }

    /// Allocate the next tab id.
    fn alloc_id(&mut self) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Create a new shell tab backed by a fresh WASM engine instance.
    pub fn create_shell_tab(
        &mut self,
        cwd: PathBuf,
        sandbox: SandboxPolicy,
    ) -> Result<usize> {
        let (mut store, instance) = super::runtime::setup(sandbox, cwd)?;

        let shell_init = instance
            .get_typed_func::<(), ()>(&mut store, "shell_init")
            .context("missing export: shell_init")?;
        let shell_eval = instance
            .get_typed_func::<u32, ()>(&mut store, "shell_eval")
            .context("missing export: shell_eval")?;
        let get_input_buf = instance
            .get_typed_func::<(), u32>(&mut store, "get_input_buf")
            .context("missing export: get_input_buf")?;
        let get_input_buf_len = instance
            .get_typed_func::<(), u32>(&mut store, "get_input_buf_len")
            .context("missing export: get_input_buf_len")?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("missing export: memory")?;

        shell_init.call(&mut store, ())?;

        let buf_ptr = get_input_buf.call(&mut store, ())? as usize;
        let buf_cap = get_input_buf_len.call(&mut store, ())? as usize;

        let id = self.alloc_id();
        let tab = Tab {
            id,
            inner: TabInner::Shell(WasmSession {
                store,
                instance,
                buf_ptr,
                buf_cap,
                shell_eval,
                memory,
            }),
            label: String::new(),
            multiline_buffer: String::new(),
            recent_commands: Vec::new(),
            ai_mode: false,
            ai_agent_id: String::from("shell"),
        };

        self.tabs.push(tab);
        let idx = self.tabs.len() - 1;
        Ok(idx)
    }

    /// Create a new AI mode tab.
    pub fn create_ai_tab(&mut self, agent_id: &str, fallback_cwd: PathBuf) -> usize {
        let id = self.alloc_id();
        let tab = Tab {
            id,
            inner: TabInner::Ai { fallback_cwd },
            label: String::new(),
            multiline_buffer: String::new(),
            recent_commands: Vec::new(),
            ai_mode: true,
            ai_agent_id: agent_id.to_string(),
        };
        self.tabs.push(tab);
        let idx = self.tabs.len() - 1;
        self.active = idx;
        idx
    }

    /// Create a new history browser tab.
    pub fn create_history_tab(&mut self, fallback_cwd: PathBuf) -> usize {
        let id = self.alloc_id();
        let tab = Tab {
            id,
            inner: TabInner::HistoryView { fallback_cwd },
            label: String::new(),
            multiline_buffer: String::new(),
            recent_commands: Vec::new(),
            ai_mode: false,
            ai_agent_id: String::new(),
        };
        self.tabs.push(tab);
        let idx = self.tabs.len() - 1;
        self.active = idx;
        idx
    }

    /// Close a tab by its index. Returns `true` if the shell should exit
    /// (last tab was closed).
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 {
            return true; // last tab — signal exit
        }
        self.tabs.remove(index);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if self.active > index {
            self.active -= 1;
        }
        false
    }

    /// Close the currently active tab.
    pub fn close_active(&mut self) -> bool {
        self.close_tab(self.active)
    }

    /// Switch to the tab at `index` (0-based).
    pub fn switch_to(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    /// Switch to the next tab (wrapping around).
    pub fn switch_next(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    /// Switch to the previous tab (wrapping around).
    pub fn switch_prev(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    /// Get a reference to the active tab.
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// Get a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Find a tab index by its numeric id.
    #[allow(dead_code)]
    pub fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }
}
