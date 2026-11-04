//! Immutable project declaration; paths are not document identities.
use super::{BibliographyTool, LatexEngine, ProjectSettings, Resource, StoreError};
mod settings;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Fixed project language, chosen when creating or importing the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectLanguage {
    /// Typst page editor or Typst source authority.
    Typst,
    /// LaTeX source authority; no Typst kernel is used.
    Latex,
}
/// The single authority selected for this project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Authority {
    /// Identified Typst document in the project database.
    Structured,
    /// Standard source file remains authoritative.
    Source,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    format: String,
    version: u32,
    id: [u8; 16],
    language: ProjectLanguage,
    authority: Authority,
    entry: PathBuf,
    resource: [u8; 16],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    engine: Option<LatexEngine>,
    #[serde(default, skip_serializing_if = "BibliographyTool::is_auto")]
    bibliography: BibliographyTool,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    entry_resources: std::collections::BTreeMap<PathBuf, [u8; 16]>,
}
/// Validated immutable project identity and entry. Does not own the project lock.
#[derive(Clone, Debug)]
pub struct ProjectHandle {
    root: PathBuf,
    declaration: Declaration,
}
impl ProjectHandle {
    /// Create a new independent project, never replacing an existing directory.
    ///
    /// # Errors
    /// Rejects existing targets and failed publication; a failed target is preserved for inspection.
    pub fn create(root: &Path, language: ProjectLanguage) -> Result<Self, StoreError> {
        Self::create_inner(root, language, false)
    }
    pub(super) fn create_migration(root: &Path) -> Result<Self, StoreError> {
        Self::create_inner(root, ProjectLanguage::Typst, true)
    }
    fn create_inner(
        root: &Path,
        language: ProjectLanguage,
        migration: bool,
    ) -> Result<Self, StoreError> {
        create_root(root)?;
        if migration {
            fs::File::create(root.join(".scholium-importing"))?.sync_all()?;
            fs::File::open(root)?.sync_all()?;
        }
        let authority = if language == ProjectLanguage::Typst {
            Authority::Structured
        } else {
            Authority::Source
        };
        let entry = if language == ProjectLanguage::Typst {
            "main.typ"
        } else {
            "main.tex"
        };
        if authority == Authority::Source {
            fs::write(
                root.join(entry),
                b"\\documentclass{article}\n\\begin{document}\n\n\\end{document}\n",
            )?;
            fs::File::open(root.join(entry))?.sync_all()?;
            fs::File::open(root)?.sync_all()?;
        }
        Self::declare(root, language, authority, Path::new(entry))
    }
    /// Open a declaration, or import an ordinary .tex/.typ file/directory without rewriting source.
    /// Mixed/ambiguous directories require an explicit project-relative entry.
    ///
    /// # Errors
    /// Rejects unsupported declarations, unsafe paths, ambiguous entries and incompatible entry choices.
    pub fn open(path: &Path, entry: Option<&Path>) -> Result<Self, StoreError> {
        let (root, selected) = if path.is_file() {
            (
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new(".")),
                path.file_name().map(Path::new),
            )
        } else {
            (path, entry)
        };
        let root = root.canonicalize()?;
        if root.join(".scholium-importing").try_exists()? {
            return Err(StoreError::Corrupt(
                "incomplete migration; source and target preserved",
            ));
        }
        let declaration = root.join(".scholium/project.toml");
        if declaration.try_exists()? {
            if fs::symlink_metadata(&declaration)?.file_type().is_symlink() {
                return Err(StoreError::InvalidResource);
            }
            let declaration: Declaration = toml::from_str(&fs::read_to_string(declaration)?)
                .map_err(|_| StoreError::Corrupt("project declaration"))?;
            let handle = Self { root, declaration };
            handle.validate()?;
            if selected.is_some_and(|e| e != handle.entry()) {
                return Err(StoreError::InvalidResource);
            }
            return Ok(handle);
        }
        let entry = match selected {
            Some(path) => path.to_path_buf(),
            None => choose_entry(&root)?,
        };
        let language = language(&entry)?;
        Self::declare(&root, language, Authority::Source, &entry)
    }
    fn declare(
        root: &Path,
        language: ProjectLanguage,
        authority: Authority,
        entry: &Path,
    ) -> Result<Self, StoreError> {
        let handle = Self {
            root: root.canonicalize()?,
            declaration: Declaration {
                format: "scholium.project".into(),
                version: 1,
                id: scholium_model::RequestId::fresh().as_bytes(),
                language,
                authority,
                entry: entry.to_path_buf(),
                resource: scholium_model::RequestId::fresh().as_bytes(),
                engine: None,
                bibliography: BibliographyTool::Auto,
                entry_resources: Default::default(),
            },
        };
        handle.validate()?;
        let report = super::ProjectStore::open(&handle.root, super::OpenPolicy::Full)?;
        // Claim a project under the same lock used by all subsequent writers.
        let meta = handle.root.join(".scholium");
        let temporary = meta.join("project.toml.initializing");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(
            toml::to_string(&handle.declaration)
                .map_err(|_| StoreError::Corrupt("project encoding"))?
                .as_bytes(),
        )?;
        file.sync_all()?;
        drop(file);
        fs::hard_link(&temporary, meta.join("project.toml"))?;
        fs::File::open(&meta)?.sync_all()?;
        fs::remove_file(temporary)?;
        fs::File::open(meta)?.sync_all()?;
        drop(report);
        Ok(handle)
    }
    fn validate(&self) -> Result<(), StoreError> {
        if self.declaration.format != "scholium.project"
            || !matches!(self.declaration.version, 1 | 2)
            || (self.authority() == Authority::Structured
                && self.language() != ProjectLanguage::Typst)
            || language(self.entry())? != self.language()
            || (self.language() == ProjectLanguage::Typst
                && (self.declaration.engine.is_some()
                    || self.declaration.bibliography != BibliographyTool::Auto))
        {
            return Err(StoreError::Corrupt("unsupported project declaration"));
        }
        self.resource()?;
        if self.declaration.version == 1
            && (self.declaration.engine.is_some()
                || self.declaration.bibliography != BibliographyTool::Auto
                || !self.declaration.entry_resources.is_empty())
        {
            return Err(StoreError::Corrupt(
                "settings require declaration version 2",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (path, id) in &self.declaration.entry_resources {
            Resource::new(*id, path)?;
            if language(path)? != self.language()
                || !seen.insert(id)
                || (path == self.entry() && *id != self.declaration.resource)
            {
                return Err(StoreError::InvalidResource);
            }
        }
        // Reject symlink components even when the eventual target stays inside the project.
        let mut path = self.root.clone();
        for part in self.entry().components() {
            path.push(part.as_os_str());
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(StoreError::InvalidResource);
                }
                Ok(_) => {}
                Err(e)
                    if self.authority() == Authority::Structured
                        && e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    /// Canonical project root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Fixed project language.
    pub fn language(&self) -> ProjectLanguage {
        self.declaration.language
    }
    /// Selected authority.
    pub fn authority(&self) -> Authority {
        self.declaration.authority
    }
    /// Project-relative entry file.
    pub fn entry(&self) -> &Path {
        &self.declaration.entry
    }
    /// Independent project identity, regenerated by Save As.
    pub fn id(&self) -> [u8; 16] {
        self.declaration.id
    }
    /// Entry's stable resource binding.
    ///
    /// # Errors
    /// Rejects invalid declaration paths.
    pub fn resource(&self) -> Result<Resource, StoreError> {
        Resource::new(self.declaration.resource, self.entry())
    }
    /// Copy a source authority into a new independent project, with the caller's selected draft bytes.
    ///
    /// # Errors
    /// Rejects existing targets, unsafe resource paths and I/O errors.
    pub fn copy_source(&self, target: &Path, bytes: &[u8]) -> Result<Self, StoreError> {
        if self.authority() != Authority::Source {
            return Err(StoreError::InvalidResource);
        }
        self.copy_into(target, Some(bytes))
    }
    /// Copy native resources to a new project, excluding private history and drafts.
    ///
    /// # Errors
    /// Rejects existing/nested targets, symlinks, special resources and failed durable copies.
    pub fn copy_native(&self, target: &Path) -> Result<Self, StoreError> {
        if self.authority() != Authority::Structured {
            return Err(StoreError::InvalidResource);
        }
        self.copy_into(target, None)
    }
    fn copy_into(&self, target: &Path, source: Option<&[u8]>) -> Result<Self, StoreError> {
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let absolute = parent
            .canonicalize()?
            .join(target.file_name().ok_or(StoreError::InvalidResource)?);
        if absolute.starts_with(&self.root) {
            return Err(StoreError::InvalidResource);
        }
        create_root(&absolute)?;
        copy_resources(&self.root, &absolute, true)?;
        if let Some(bytes) = source {
            let path = absolute.join(self.entry());
            fs::write(&path, bytes)?;
            fs::File::open(path)?.sync_all()?;
        }
        fs::File::open(&absolute)?.sync_all()?;
        let handle = Self::declare(&absolute, self.language(), self.authority(), self.entry())?;
        if self.authority() == Authority::Source
            && (self.declaration.engine.is_some()
                || self.declaration.bibliography != BibliographyTool::Auto)
        {
            let mut copy = super::SourceProject::open(handle)?;
            copy.update_settings(self.settings())?;
            Ok(copy.handle().clone())
        } else {
            Ok(handle)
        }
    }
}
fn language(path: &Path) -> Result<ProjectLanguage, StoreError> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("typ") => Ok(ProjectLanguage::Typst),
        Some("tex") => Ok(ProjectLanguage::Latex),
        _ => Err(StoreError::InvalidResource),
    }
}
fn choose_entry(root: &Path) -> Result<PathBuf, StoreError> {
    let mut choices = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && language(&entry.path()).is_ok() {
            choices.push(PathBuf::from(entry.file_name()));
        }
    }
    if choices.len() != 1 {
        return Err(StoreError::EntryRequired);
    }
    Ok(choices.remove(0))
}

fn copy_resources(source: &Path, target: &Path, root: bool) -> Result<(), StoreError> {
    for item in fs::read_dir(source)? {
        let item = item?;
        if root && item.file_name() == ".scholium" {
            continue;
        }
        let kind = item.file_type()?;
        let path = target.join(item.file_name());
        if kind.is_dir() {
            fs::create_dir(&path)?;
            copy_resources(&item.path(), &path, false)?;
            fs::File::open(path)?.sync_all()?;
        } else if kind.is_file() {
            fs::copy(item.path(), &path)?;
            fs::File::open(path)?.sync_all()?;
        } else {
            return Err(StoreError::InvalidResource);
        }
    }
    Ok(())
}

fn create_root(root: &Path) -> Result<(), StoreError> {
    fs::create_dir(root)?;
    let parent = root
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod settings_tests;
