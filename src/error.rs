#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation failed")]
    Failed,

    #[error("not found: {0}")]
    NotFound(String),

    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    #[error(transparent)]
    Format(#[from] std::fmt::Error),

    #[error(transparent)]
    Open(#[from] Box<gix::open::Error>),

    #[error(transparent)]
    ObjectFind(#[from] gix::object::find::existing::Error),

    #[error(transparent)]
    ObjectFindConversion(#[from] gix::object::find::existing::with_conversion::Error),

    #[error(transparent)]
    ObjectCommit(#[from] gix::object::commit::Error),

    #[error(transparent)]
    ObjectTree(#[from] gix::object::tree::diff::for_each::Error),

    #[error(transparent)]
    DiffOptions(#[from] gix::diff::options::init::Error),

    #[error(transparent)]
    DiffPrepare(#[from] gix::diff::blob::platform::prepare_diff::Error),

    #[error(transparent)]
    DiffResource(#[from] gix::diff::blob::platform::set_resource::Error),

    #[error(transparent)]
    RepoDiff(#[from] gix::repository::diff_resource_cache::Error),

    #[error(transparent)]
    DecodeCommit(#[from] gix::objs::decode::Error),

    #[error(transparent)]
    DecodeHash(#[from] gix::hash::decode::Error),

    #[error(transparent)]
    RevWalk(#[from] gix::revision::walk::Error),

    #[error(transparent)]
    RevWalkIter(#[from] gix::revision::walk::iter::Error),

    #[error(transparent)]
    RevParseSpec(#[from] gix::revision::spec::parse::single::Error),

    #[error(transparent)]
    PeelToCommit(#[from] gix::head::peel::to_commit::Error),

    #[error(transparent)]
    PeelToKind(#[from] gix::reference::peel::to_kind::Error),

    #[error(transparent)]
    ReferenceFind(#[from] gix::reference::find::existing::Error),

    #[error(transparent)]
    ReferenceIter(#[from] gix::reference::iter::Error),

    #[error(transparent)]
    ReferenceIterInit(#[from] gix::reference::iter::init::Error),

    #[error(transparent)]
    Boxed(#[from] Box<dyn std::error::Error + Send + Sync>),
}
