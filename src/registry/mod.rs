pub mod dimension_type;
pub mod tags;

use crate::util::ident::Identifier;

pub use tags::RegistryTags;

pub trait RegistryEntry: PartialEq + 'static {
    fn key(&self) -> &Identifier;
    fn try_id(&self) -> Option<usize>;

    fn id(&self) -> usize {
        self.try_id().expect("entry not found in registry")
    }
}

pub trait RegistryExt {
    type Entry: RegistryEntry;

    fn freeze(&mut self);
    fn by_id(&self, id: usize) -> Option<&'static Self::Entry>;
    fn by_key(&self, key: &Identifier) -> Option<&'static Self::Entry>;
    fn id_from_key(&self, key: &Identifier) -> Option<usize>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

pub trait TaggedRegistryExt: RegistryExt {
    fn register_tag(&mut self, tag: Identifier, keys: &[&'static str]);
    fn modify_tag(&mut self, tag: &Identifier, f: impl FnOnce(Vec<Identifier>) -> Vec<Identifier>);
    fn is_in_tag(&self, entry: &'static Self::Entry, tag: &Identifier) -> bool;
    fn get_tag(&self, tag: &Identifier) -> Option<Vec<&'static Self::Entry>>;
    fn iter_tag(&self, tag: &Identifier) -> impl Iterator<Item = &'static Self::Entry> + '_;
    fn tag_keys(&self) -> impl Iterator<Item = &Identifier> + '_;
}
