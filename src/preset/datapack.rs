use shulkr::util::Key;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatapackPreset {
    key: Key,
}

impl DatapackPreset {
    pub const fn new(key: Key) -> Self {
        Self { key }
    }

    pub const fn key(&self) -> &Key {
        &self.key
    }
}
