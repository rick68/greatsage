//! REPL prompt input history: in-memory recall and line-oriented persistence.

use {
    bevy::ecs::resource::Resource,
    std::{
        fs, io,
        path::{Path, PathBuf},
    },
};

pub const DEFAULT_MAX_ENTRIES: usize = 1000;

#[derive(Resource, Debug)]
pub struct ReplInputHistory {
    entries: Vec<String>,
    recall_index: Option<usize>,
    draft: Option<String>,
    max_entries: usize,
    dirty: bool,
}

impl ReplInputHistory {
    pub fn load(path: &Path, max_entries: usize) -> Self {
        let entries = load_entries_from_file(path, max_entries);
        Self {
            entries,
            recall_index: None,
            draft: None,
            max_entries,
            dirty: false,
        }
    }

    pub fn save_if_dirty(&mut self, path: &Path) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut body = String::new();
        for entry in &self.entries {
            () = body.push_str(entry);
            () = body.push('\n');
        }
        let tmp = path
            .parent()
            .map(|dir| dir.join(".history.tmp"))
            .unwrap_or_else(|| PathBuf::from(".history.tmp"));
        () = fs::write(&tmp, body)?;
        () = fs::rename(tmp, path)?;
        self.dirty = false;
        Ok(())
    }

    pub fn push_submitted(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }
        if self.entries.last().is_some_and(|last| last == line) {
            () = self.cancel_recall();
            return;
        }
        () = self.entries.push(line.to_owned());
        () = self.trim_to_cap();
        self.dirty = true;
        () = self.cancel_recall();
    }

    pub fn recall_up(&mut self, current_line: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let last = self.entries.len() - 1;
        match self.recall_index {
            None => {
                self.draft = Some(current_line.to_owned());
                self.recall_index = Some(last);
            }
            Some(index) if index > 0 => {
                self.recall_index = Some(index - 1);
            }
            Some(index) => {
                self.recall_index = Some(index);
            }
        }
        self.entries.get(self.recall_index?).cloned()
    }

    pub fn recall_down(&mut self) -> Option<String> {
        let index = self.recall_index?;
        let last = self.entries.len().saturating_sub(1);
        if index < last {
            self.recall_index = Some(index + 1);
            () = return self.entries.get(self.recall_index?).cloned();
        }
        self.recall_index = None;
        Some(self.draft.take().unwrap_or_default())
    }

    pub fn cancel_recall(&mut self) {
        self.recall_index = None;
        self.draft = None;
    }

    pub fn is_recalling(&self) -> bool {
        self.recall_index.is_some()
    }

    #[allow(dead_code)]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    fn trim_to_cap(&mut self) {
        if self.entries.len() > self.max_entries {
            let excess = self.entries.len() - self.max_entries;
            self.entries.drain(0..excess);
            if let Some(index) = self.recall_index {
                if index < excess {
                    self.recall_index = None;
                    self.draft = None;
                } else {
                    self.recall_index = Some(index - excess);
                }
            }
        }
    }
}

fn load_entries_from_file(path: &Path, max_entries: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        entries.push(line.to_owned());
    }
    if entries.len() > max_entries {
        let excess = entries.len() - max_entries;
        entries.drain(0..excess);
    }
    entries
}

pub fn persist_repl_history(history: &mut ReplInputHistory, path: &Path) {
    let _ = history.save_if_dirty(path);
}
