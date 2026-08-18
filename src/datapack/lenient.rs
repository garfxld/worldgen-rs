use std::vec::IntoIter;

use serde::de::{
    DeserializeSeed, Deserializer, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use serde_json::map::IntoIter as MapIntoIter;
use serde_json::{Error, Map, Value};

pub struct Lenient(pub Value);

macro_rules! truncating {
    ($($method:ident => $visit:ident as $ty:ty),* $(,)?) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
                match &self.0 {
                    Value::Number(number) => match number.as_f64() {
                        Some(value) => visitor.$visit(value as $ty),
                        None => self.0.$method(visitor),
                    },
                    _ => self.0.$method(visitor),
                }
            }
        )*
    };
}

impl<'de> Deserializer<'de> for Lenient {
    type Error = Error;

    truncating! {
        deserialize_i8 => visit_i64 as i64,
        deserialize_i16 => visit_i64 as i64,
        deserialize_i32 => visit_i64 as i64,
        deserialize_i64 => visit_i64 as i64,
        deserialize_u8 => visit_i64 as i64,
        deserialize_u16 => visit_i64 as i64,
        deserialize_u32 => visit_i64 as i64,
        deserialize_u64 => visit_i64 as i64,
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            Value::Array(items) => visitor.visit_seq(LenientSeq(items.into_iter())),
            Value::Object(entries) => visitor.visit_map(LenientMap::new(entries)),
            other => other.deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0 {
            Value::Null => visitor.visit_none(),
            other => visitor.visit_some(Self(other)),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.0 {
            Value::Object(entries) if entries.len() == 1 => {
                let (variant, value) = entries.into_iter().next().expect("one entry");
                visitor.visit_enum(LenientEnum { variant, value })
            }
            Value::String(_) => self.0.deserialize_enum(name, variants, visitor),
            other => other.deserialize_enum(name, variants, visitor),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_bool(visitor)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_f32(visitor)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_f64(visitor)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_char(visitor)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_str(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_string(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_bytes(visitor)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_byte_buf(visitor)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_unit(visitor)
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.0.deserialize_unit_struct(name, visitor)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_identifier(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.0.deserialize_ignored_any(visitor)
    }
}

struct LenientSeq(IntoIter<Value>);

impl<'de> SeqAccess<'de> for LenientSeq {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.0.next() {
            Some(value) => seed.deserialize(Lenient(value)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.0.len())
    }
}

struct LenientMap {
    entries: MapIntoIter,
    value: Option<Value>,
}

impl LenientMap {
    fn new(entries: Map<String, Value>) -> Self {
        Self {
            entries: entries.into_iter(),
            value: None,
        }
    }
}

impl<'de> MapAccess<'de> for LenientMap {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.entries.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(key.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let value = self.value.take().expect("value follows its key");
        seed.deserialize(Lenient(value))
    }
}

struct LenientEnum {
    variant: String,
    value: Value,
}

impl<'de> EnumAccess<'de> for LenientEnum {
    type Error = Error;
    type Variant = LenientVariant;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((variant, LenientVariant(self.value)))
    }
}

struct LenientVariant(Value);

impl<'de> VariantAccess<'de> for LenientVariant {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(Lenient(self.0))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        Lenient(self.0).deserialize_any(visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        Lenient(self.0).deserialize_any(visitor)
    }
}
