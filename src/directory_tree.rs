use crate::re_client::Digest;
use std::path::PathBuf;
use std::io;
use nativelink_proto::build::bazel::remote::execution::v2 as cas;
use prost::Message;

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

#[derive(Default)]
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

            // TODO: This only extracts basename. For nested directories,
            // we'll need to preserve relative paths to prevent collisions.
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

        // Sort files lexicographically (required by RE API v2)
        file_nodes.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Directory {
            files: file_nodes,
            directories: vec![],
        })
    }
}

impl Directory {
    pub fn to_proto(&self) -> cas::Directory {
        cas::Directory {
            files: self.files.iter().map(|f| cas::FileNode {
                name: f.name.clone(),
                digest: Some(cas::Digest {
                    hash: f.digest.hash.clone(),
                    size_bytes: f.digest.size_bytes,
                }),
                is_executable: f.is_executable,
                node_properties: None,
            }).collect(),
            directories: self.directories.iter().map(|d| cas::DirectoryNode {
                name: d.name.clone(),
                digest: Some(cas::Digest {
                    hash: d.digest.hash.clone(),
                    size_bytes: d.digest.size_bytes,
                }),
            }).collect(),
            symlinks: vec![],
            node_properties: None,
        }
    }

    pub fn to_proto_bytes(&self) -> io::Result<Vec<u8>> {
        let proto = self.to_proto();
        let mut buf = Vec::new();
        proto.encode(&mut buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(buf)
    }
}
