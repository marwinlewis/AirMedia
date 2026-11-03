use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct NativeFileDialog {
    filters: Vec<(String, Vec<String>)>,
    directory: Option<PathBuf>,
    file_name: Option<String>,
}

impl NativeFileDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.filters.push((
            name.to_string(),
            extensions.iter().map(|s| s.to_string()).collect(),
        ));
        self
    }

    pub fn set_directory<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.directory = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn set_file_name(mut self, name: &str) -> Self {
        self.file_name = Some(name.to_string());
        self
    }

    pub fn save_file(self) -> Option<PathBuf> {
        #[cfg(not(target_os = "android"))]
        {
            let mut dialog = rfd::FileDialog::new();
            if let Some(ref dir) = self.directory {
                dialog = dialog.set_directory(dir);
            }
            if let Some(ref name) = self.file_name {
                dialog = dialog.set_file_name(name);
            }
            for (name, exts) in &self.filters {
                let str_exts: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
                dialog = dialog.add_filter(name, &str_exts);
            }
            dialog.save_file()
        }
        #[cfg(target_os = "android")]
        {
            None
        }
    }

    pub fn pick_file(self) -> Option<PathBuf> {
        #[cfg(not(target_os = "android"))]
        {
            let mut dialog = rfd::FileDialog::new();
            if let Some(ref dir) = self.directory {
                dialog = dialog.set_directory(dir);
            }
            for (name, exts) in &self.filters {
                let str_exts: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
                dialog = dialog.add_filter(name, &str_exts);
            }
            dialog.pick_file()
        }
        #[cfg(target_os = "android")]
        {
            None
        }
    }

    pub fn pick_files(self) -> Option<Vec<PathBuf>> {
        #[cfg(not(target_os = "android"))]
        {
            let mut dialog = rfd::FileDialog::new();
            if let Some(ref dir) = self.directory {
                dialog = dialog.set_directory(dir);
            }
            for (name, exts) in &self.filters {
                let str_exts: Vec<&str> = exts.iter().map(|s| s.as_str()).collect();
                dialog = dialog.add_filter(name, &str_exts);
            }
            dialog.pick_files()
        }
        #[cfg(target_os = "android")]
        {
            None
        }
    }

    pub fn pick_folder(self) -> Option<PathBuf> {
        #[cfg(not(target_os = "android"))]
        {
            let mut dialog = rfd::FileDialog::new();
            if let Some(ref dir) = self.directory {
                dialog = dialog.set_directory(dir);
            }
            dialog.pick_folder()
        }
        #[cfg(target_os = "android")]
        {
            None
        }
    }
}
