use alloc::borrow::Cow;

pub(crate) static ANCHOR: () = ();

/// Defines how the `World` should be serialized.
#[derive(Clone, Copy)]
pub struct GlobalSerConfig {
    pub same_binary: bool,
    pub with_entities: bool,
    pub with_shared: WithShared,
}

impl Default for GlobalSerConfig {
    fn default() -> Self {
        GlobalSerConfig {
            same_binary: true,
            with_entities: true,
            with_shared: WithShared::PerStorage,
        }
    }
}

/// Defines how the `World` should be deserialized.
#[derive(Clone, Copy)]
pub struct GlobalDeConfig {
    pub existing_entities: ExistingEntities,
    pub with_shared: WithShared,
}

impl Default for GlobalDeConfig {
    fn default() -> Self {
        GlobalDeConfig {
            existing_entities: ExistingEntities::AsNew,
            with_shared: WithShared::PerStorage,
        }
    }
}

/// Describes what the deserialize process should do when it encounters an already existing entity.
/// - AsNew will deserialize the entity's components with a new `EntityId`
/// - Merge will only deserialize the components the entity didn't have
/// - Replace will delete all components present and add the ones from the deserializetion
/// - Skip will not deserialize this entity
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExistingEntities {
    AsNew,
    Merge,
    Replace,
    Skip,
}

/// Describes how shared components should be (de)seriliazed.
/// - All will (de)serialize shared component for all storages
/// - None will not (de)serialize shared components for any storage
/// - PerStorage will (de)serailize shared components following each storage's config
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WithShared {
    All,
    None,
    PerStorage,
}

pub struct Identifier(Cow<'static, str>);

impl Identifier {
    pub fn new<I: Into<Cow<'static, str>>>(identifier: I) -> Self {
        Identifier(identifier.into())
    }
}

/// Defines how a storage should be serialized.
pub struct SerConfig {
    pub identifier: Option<Identifier>,
    pub with_shared: bool,
}

impl Default for SerConfig {
    fn default() -> Self {
        SerConfig {
            identifier: None,
            with_shared: false,
        }
    }
}
