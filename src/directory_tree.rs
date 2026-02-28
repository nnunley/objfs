use crate::re_client::Digest;
use std::path::PathBuf;
use std::io;

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub digest: Digest,
    pub is_executable: bool,
}

#[derive(Debug, Clone)]
pub struct Directory {
    pub files: Vec<FileNode>,
    pub directories: Vec<DirectoryNode>,
}

#[derive(Debug, Clone)]
pub struct DirectoryNode {
    pub name: String,
    pub digest: Digest,
}

pub struct DirectoryTreeBuilder;

impl DirectoryTreeBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, files: &[PathBuf]) -> io::Result<Directory> {
        let mut file_nodes = Vec::new();

        for file_path in files {
            if !file_path.exists() {
                continue;
            }

            let contents = std::fs::read(file_path)?;
            let digest = Digest::from_data(&contents);

            let name = file_path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No filename"))?
                .to_string_lossy()
                .to_string();

            #[cfg(unix)]
            let is_executable = {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(file_path)?.permissions().mode() & 0o111 != 0
            };

            #[cfg(not(unix))]
            let is_executable = false;

            file_nodes.push(FileNode {
                name,
                digest,
                is_executable,
            });
        }

        Ok(Directory {
            files: file_nodes,
            directories: vec![],
        })
    }
}
