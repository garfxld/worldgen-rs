use std::borrow::Cow;
use std::fmt::{self, Debug, Display, Formatter};
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Identifier {
    pub namespace: Cow<'static, str>,
    pub path: Cow<'static, str>,
}

impl Debug for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&format!("{}:{}", self.namespace, self.path))
    }
}

impl Identifier {
    pub const VANILLA_NAMESPACE: &'static str = "minecraft";

    pub fn new(
        namespace: impl Into<Cow<'static, str>>,
        path: impl Into<Cow<'static, str>>,
    ) -> Self {
        Identifier {
            namespace: namespace.into(),
            path: path.into(),
        }
    }

    pub const fn const_new(namespace: &'static str, path: &'static str) -> Self {
        Identifier {
            namespace: Cow::Borrowed(namespace),
            path: Cow::Borrowed(path),
        }
    }

    pub const fn vanilla(path: String) -> Self {
        Identifier {
            namespace: Cow::Borrowed(Self::VANILLA_NAMESPACE),
            path: Cow::Owned(path),
        }
    }

    pub const fn const_vanilla(path: &'static str) -> Self {
        Identifier {
            namespace: Cow::Borrowed(Self::VANILLA_NAMESPACE),
            path: Cow::Borrowed(path),
        }
    }

    pub const fn valid_namespace_char(char: char) -> bool {
        char == '_'
            || char == '-'
            || char.is_ascii_lowercase()
            || char.is_ascii_digit()
            || char == '.'
    }

    pub const fn valid_char(char: char) -> bool {
        Self::valid_namespace_char(char) || char == '/'
    }

    pub fn validate_namespace(namespace: &str) -> bool {
        namespace != ".." && namespace.chars().all(Self::valid_namespace_char)
    }

    pub fn validate_path(path: &str) -> bool {
        path.chars().all(Self::valid_char)
    }

    pub fn validate(namespace: &str, path: &str) -> bool {
        Self::validate_namespace(namespace) && Self::validate_path(path)
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

impl FromStr for Identifier {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (namespace, path) = match s.split_once(':') {
            Some(("", path)) => (Self::VANILLA_NAMESPACE, path),
            Some((namespace, path)) => (namespace, path),
            None => (Self::VANILLA_NAMESPACE, s),
        };

        if !Identifier::validate_namespace(namespace) {
            return Err("Invalid namespace");
        }

        if !Identifier::validate_path(path) {
            return Err("Invalid path");
        }

        Ok(Identifier {
            namespace: Cow::Owned(namespace.to_owned()),
            path: Cow::Owned(path.to_owned()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BlockState(pub u16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaced_and_bare_paths() {
        assert_eq!(
            "minecraft:stone".parse::<Identifier>().expect("test value"),
            Identifier::new("minecraft", "stone")
        );
        assert_eq!(
            "stone".parse::<Identifier>().expect("test value"),
            Identifier::new("minecraft", "stone")
        );
        assert_eq!(
            ":stone".parse::<Identifier>().expect("test value"),
            Identifier::new("minecraft", "stone")
        );
    }

    #[test]
    fn rejects_invalid_namespace_and_path() {
        assert!("Minecraft:stone".parse::<Identifier>().is_err());
        assert!("minecraft:Stone".parse::<Identifier>().is_err());
        assert!("..:stone".parse::<Identifier>().is_err());
    }

    #[test]
    fn display_roundtrips_through_from_str() {
        let id = Identifier::new("modid", "some/nested/path");
        let text = id.to_string();
        assert_eq!(text, "modid:some/nested/path");
        assert_eq!(text.parse::<Identifier>().expect("test value"), id);
    }

    #[test]
    fn const_constructors_match_runtime_equivalents() {
        assert_eq!(
            Identifier::const_new("minecraft", "dirt"),
            Identifier::new("minecraft", "dirt")
        );
        assert_eq!(
            Identifier::const_vanilla("dirt"),
            Identifier::new("minecraft", "dirt")
        );
    }
}
