use serde::{Deserialize, Deserializer, Serializer};

/// Allows to serializing, deserializing a `Vec<u8>` as base64.
///
/// ```ignore
/// # use serde::{Serialize, Deserialize};
/// #[derive(Serialize, Deserialize)]
/// struct Foo {
///     #[serde(
///         serialize_with = "util::to_base64",
///         deserialize_with = "util::from_base64"
///     )]
///     data: Vec<u8>
/// }
/// ```
#[allow(clippy::ptr_arg)]
pub fn to_base64<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&base64::encode(&data[..]))
}

/// Allows to serializing, deserializing a `Vec<u8>` as base64.
///
/// ```ignore
/// # use serde::{Serialize, Deserialize};
/// #[derive(Serialize, Deserialize)]
/// struct Foo {
///     #[serde(
///         serialize_with = "util::to_base64",
///         deserialize_with = "util::from_base64"
///     )]
///     data: Vec<u8>
/// }
/// ```
pub fn from_base64<'a, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'a>,
{
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| base64::decode(&string).map_err(|err| Error::custom(err.to_string())))
}
