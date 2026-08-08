use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Identifier {
    pub namespace: Cow<'static, str>,
    pub path: Cow<'static, str>,
}

impl Identifier {
    pub const VANILLA_NAMESPACE: &'static str = "minecraft";

    #[must_use]
    pub const fn const_new(namespace: &'static str, path: &'static str) -> Self {
        Identifier {
            namespace: Cow::Borrowed(namespace),
            path: Cow::Borrowed(path),
        }
    }

    #[must_use]
    pub const fn const_vanilla(path: &'static str) -> Self {
        Self::const_new(Self::VANILLA_NAMESPACE, path)
    }
}
