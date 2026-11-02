//! Wire strings with transparent Borsh representations. Hex input is strictly lowercase.
use base64::{Engine, engine::general_purpose::STANDARD};
use schemars::{
    JsonSchema,
    r#gen::SchemaGenerator,
    schema::{InstanceType, Schema, SchemaObject, StringValidation},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct Hex20(pub [u8; 20]);
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct Hex32(pub [u8; 32]);
#[derive(Debug, Clone, PartialEq, Eq, Hash, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct HexBytes(pub Vec<u8>);
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct U128Str(pub u128);
/// Standard padded base64. `Base64<32>` is a public key; `Base64<64>` is a signature.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, borsh::BorshSerialize, borsh::BorshDeserialize,
)]
pub struct Base64<const N: usize>(pub [u8; N]);
/// Standard padded base64 carrying unmodified canonical certificate bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Base64Bytes(pub Vec<u8>);

// The Borsh schema describes bytes, not the JSON-facing wrapper name or shape.
macro_rules! transparent_borsh_schema {
    ($wrapper:ty, $raw:ty) => {
        impl borsh::BorshSchema for $wrapper {
            fn declaration() -> borsh::schema::Declaration {
                <$raw as borsh::BorshSchema>::declaration()
            }
            fn add_definitions_recursively(
                definitions: &mut std::collections::BTreeMap<
                    borsh::schema::Declaration,
                    borsh::schema::Definition,
                >,
            ) {
                <$raw as borsh::BorshSchema>::add_definitions_recursively(definitions);
            }
        }
    };
}

transparent_borsh_schema!(Hex20, [u8; 20]);
transparent_borsh_schema!(Hex32, [u8; 32]);
transparent_borsh_schema!(HexBytes, Vec<u8>);
transparent_borsh_schema!(U128Str, u128);
transparent_borsh_schema!(Base64Bytes, Vec<u8>);

impl<const N: usize> borsh::BorshSchema for Base64<N> {
    fn declaration() -> borsh::schema::Declaration {
        <[u8; N] as borsh::BorshSchema>::declaration()
    }
    fn add_definitions_recursively(
        definitions: &mut std::collections::BTreeMap<
            borsh::schema::Declaration,
            borsh::schema::Definition,
        >,
    ) {
        <[u8; N] as borsh::BorshSchema>::add_definitions_recursively(definitions);
    }
}

fn serialize_hex<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    struct HexDisplay<'a>(&'a [u8]);
    impl std::fmt::Display for HexDisplay<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("0x")?;
            for byte in self.0 {
                write!(f, "{byte:02x}")?;
            }
            Ok(())
        }
    }
    serializer.collect_str(&HexDisplay(bytes))
}

fn hex_digits(text: &str) -> Result<&[u8], &'static str> {
    let digits = text
        .strip_prefix("0x")
        .ok_or("expected lowercase 0x prefix")?
        .as_bytes();
    if !digits.len().is_multiple_of(2)
        || !digits
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    {
        return Err("expected complete lowercase hex bytes");
    }
    Ok(digits)
}

fn hex_byte(pair: &[u8; 2]) -> u8 {
    let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
    (nibble(pair[0]) << 4) | nibble(pair[1])
}

// Deserialize directly from the JSON string without an intermediate owned String.
fn parse_string<'de, D, T, F>(deserializer: D, parse: F) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    F: FnOnce(&str) -> Result<T, &'static str>,
{
    struct StringVisitor<F>(F);
    impl<'de, T, F: FnOnce(&str) -> Result<T, &'static str>> serde::de::Visitor<'de>
        for StringVisitor<F>
    {
        type Value = T;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a canonical wire string")
        }
        fn visit_str<E: Error>(self, value: &str) -> Result<T, E> {
            (self.0)(value).map_err(E::custom)
        }
    }
    deserializer.deserialize_str(StringVisitor(parse))
}

fn string_schema(pattern: Option<&str>, length: Option<u32>, format: Option<&str>) -> Schema {
    SchemaObject {
        instance_type: Some(InstanceType::String.into()),
        format: format.map(str::to_owned),
        string: Some(Box::new(StringValidation {
            pattern: pattern.map(str::to_owned),
            min_length: length,
            max_length: length,
        })),
        ..Default::default()
    }
    .into()
}

macro_rules! fixed_hex {
    ($name:ident, $size:literal) => {
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serialize_hex(&self.0, serializer)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                parse_string(deserializer, |text| {
                    if text.len() != 2 + $size * 2 {
                        return Err("wrong hex length");
                    }
                    let digits = hex_digits(text)?;
                    let mut bytes = [0; $size];
                    for (byte, pair) in bytes.iter_mut().zip(digits.as_chunks::<2>().0.iter()) {
                        *byte = hex_byte(pair);
                    }
                    Ok(Self(bytes))
                })
            }
        }
        impl JsonSchema for $name {
            fn schema_name() -> String {
                stringify!($name).into()
            }
            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                string_schema(Some("^0x[0-9a-f]*$"), Some(2 + $size * 2), None)
            }
        }
    };
}
fixed_hex!(Hex20, 20);
fixed_hex!(Hex32, 32);

impl Serialize for HexBytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_hex(&self.0, serializer)
    }
}
impl<'de> Deserialize<'de> for HexBytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        parse_string(deserializer, |text| {
            Ok(Self(
                hex_digits(text)?
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(hex_byte)
                    .collect(),
            ))
        })
    }
}
impl JsonSchema for HexBytes {
    fn schema_name() -> String {
        "HexBytes".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        string_schema(Some("^0x([0-9a-f]{2})*$"), None, None)
    }
}
impl Serialize for U128Str {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}
impl<'de> Deserialize<'de> for U128Str {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        parse_string(deserializer, |text| {
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err("expected unsigned decimal string");
            }
            text.parse()
                .map(Self)
                .map_err(|_| "decimal value exceeds u128")
        })
    }
}
impl JsonSchema for U128Str {
    fn schema_name() -> String {
        "U128Str".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        string_schema(Some("^[0-9]+$"), None, None)
    }
}
impl<const N: usize> Serialize for Base64<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&base64::display::Base64Display::new(&self.0, &STANDARD))
    }
}
impl<'de, const N: usize> Deserialize<'de> for Base64<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        parse_string(deserializer, |text| {
            let mut bytes = [0; N];
            let length = STANDARD
                .decode_slice(text, &mut bytes)
                .map_err(|_| "invalid base64 or byte length")?;
            if length != N {
                return Err("wrong base64 byte length");
            }
            Ok(Self(bytes))
        })
    }
}
impl<const N: usize> JsonSchema for Base64<N> {
    fn schema_name() -> String {
        format!("Base64_{N}")
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        string_schema(None, Some((N.div_ceil(3) * 4) as u32), Some("byte"))
    }
}
impl Serialize for Base64Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&base64::display::Base64Display::new(&self.0, &STANDARD))
    }
}
impl<'de> Deserialize<'de> for Base64Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        parse_string(deserializer, |text| {
            STANDARD
                .decode(text)
                .map(Self)
                .map_err(|_| "invalid base64")
        })
    }
}
impl JsonSchema for Base64Bytes {
    fn schema_name() -> String {
        "Base64Bytes".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        string_schema(None, None, Some("byte"))
    }
}

macro_rules! fixed_adapter {
    ($module:ident, $wrapper:ident, $raw:ty) => {
        pub(crate) mod $module {
            use super::*;
            pub fn serialize<S: Serializer>(
                value: &$raw,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                $wrapper(*value).serialize(serializer)
            }
            pub fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<$raw, D::Error> {
                $wrapper::deserialize(deserializer).map(|value| value.0)
            }
        }
    };
}
fixed_adapter!(hex20, Hex20, [u8; 20]);
fixed_adapter!(hex32, Hex32, [u8; 32]);
fixed_adapter!(u128_str, U128Str, u128);
fixed_adapter!(base64_32, Base64, [u8; 32]);

pub(crate) mod hex_bytes {
    use super::*;
    pub fn serialize<S: Serializer>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serialize_hex(value, serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        HexBytes::deserialize(deserializer).map(|value| value.0)
    }
}
pub(crate) mod optional_hex32 {
    use super::*;
    pub fn serialize<S: Serializer>(
        value: &Option<[u8; 32]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Hex32).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<[u8; 32]>, D::Error> {
        Option::<Hex32>::deserialize(deserializer).map(|value| value.map(|value| value.0))
    }
}
pub(crate) mod optional_hex_bytes {
    use super::*;
    struct BorrowedHex<'a>(&'a [u8]);
    impl Serialize for BorrowedHex<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serialize_hex(self.0, serializer)
        }
    }
    pub fn serialize<S: Serializer>(
        value: &Option<Vec<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_deref().map(BorrowedHex).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Vec<u8>>, D::Error> {
        Option::<HexBytes>::deserialize(deserializer).map(|value| value.map(|value| value.0))
    }
}
