/// A rejected adapter operation. Error messages never contain document text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An existing structural planner rejected the request.
    #[error(transparent)]
    Edit(#[from] scholium_document::EditError),
    /// The supplied structured document is invalid.
    #[error(transparent)]
    Structure(#[from] scholium_model::structured::StructureError),
    /// A binary envelope, version or internal schema is invalid.
    #[error("invalid SDG envelope or schema")]
    Format,
    /// The document, actor or node identity does not match this replica.
    #[error("SDG identity mismatch")]
    Identity,
    /// The action ID is already used for different bytes.
    #[error("SDG action identity reused")]
    Duplicate,
    /// An envelope or graph exceeds declared capacity.
    #[error("SDG capacity exceeded")]
    Capacity,
    /// Loro rejected an operation; the engine's message is not exposed as UI text.
    #[error("SDG engine rejected operation")]
    Engine,
}
impl From<loro::LoroError> for Error {
    fn from(_: loro::LoroError) -> Self {
        Self::Engine
    }
}
impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::Format
    }
}
