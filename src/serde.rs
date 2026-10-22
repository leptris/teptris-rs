//! serde support: `Value`/`Table`/`Datetime` implement both traits,
//! and [`from_str`]/[`to_string`] convert between TOML text and any
//! `Deserialize`/`Serialize` type — the ecosystem table stakes.
//!
//! Datetime convention (matching the `toml` crate so the two are
//! drop-in interchangeable): a `Datetime` serializes as the
//! one-entry map `{"$__toml_private_datetime": "<RFC 3339-ish>"}`,
//! and deserializes from that map. In formats without TOML datetime
//! semantics (e.g. JSON) datetimes therefore round-trip as that map.

use std::fmt;

use serde::de::{self, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};

use crate::{Datetime, DatetimeKind, Error, ParseError, Table, Value};

const DATETIME_KEY: &str = "$__toml_private_datetime";

/// The serde paths' error type: a custom message, a parse failure,
/// or an engine status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerdeError {
    Custom(String),
    Parse(ParseError),
    Status(Error),
}

impl fmt::Display for SerdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerdeError::Custom(m) => f.write_str(m),
            SerdeError::Parse(e) => write!(f, "{e}"),
            SerdeError::Status(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SerdeError {}

impl SerdeError {
    fn custom(msg: impl fmt::Display) -> Self {
        SerdeError::Custom(msg.to_string())
    }
}

impl de::Error for SerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerdeError::Custom(msg.to_string())
    }
}

impl ser::Error for SerdeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        SerdeError::Custom(msg.to_string())
    }
}

use serde::ser;

impl From<ParseError> for SerdeError {
    fn from(e: ParseError) -> Self {
        SerdeError::Parse(e)
    }
}

impl From<Error> for SerdeError {
    fn from(e: Error) -> Self {
        SerdeError::Status(e)
    }
}

/// Parse TOML text into any `Deserialize` type — the `toml` crate's
/// `from_str`, at engine speed.
pub fn from_str<T>(s: &str) -> Result<T, SerdeError>
where
    T: for<'de> de::Deserialize<'de>,
{
    let value = crate::loads(s.as_bytes())?;
    Ok(T::deserialize(ValueDeserializer { value })?)
}

/// Serialize any `Serialize` type as TOML text. TOML has no null:
/// `Option::None` is a serialization error (match the `toml`
/// crate's contract; use `#[serde(skip_serializing_if)]` to omit).
pub fn to_string<T>(v: &T) -> Result<String, SerdeError>
where
    T: ser::Serialize + ?Sized,
{
    let value = v.serialize(ValueSerializer)?;
    Ok(crate::dump(&value)?)
}

// -----------------------------------------------------------------
// Serialize (Value/Table/Datetime -> serde)
// -----------------------------------------------------------------

impl ser::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match self {
            Value::String(s) => serializer.serialize_str(s),
            Value::Integer(i) => serializer.serialize_i64(*i),
            Value::Float(x) => serializer.serialize_f64(*x),
            Value::Boolean(b) => serializer.serialize_bool(*b),
            Value::Datetime(dt) => dt.serialize(serializer),
            Value::Array(a) => serializer.collect_seq(a),
            Value::Table(t) => t.serialize(serializer),
        }
    }
}

impl ser::Serialize for Table {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self.iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl ser::Serialize for Datetime {
    /// The `toml`-crate convention: the one-entry private map, so
    /// deserializers in any self-describing format can recognize a
    /// TOML datetime and round-trip it.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(DATETIME_KEY, &self.to_string())?;
        map.end()
    }
}

// -----------------------------------------------------------------
// Deserialize (serde -> Value/Table/Datetime)
// -----------------------------------------------------------------

impl<'de> de::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> de::Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("any TOML value")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
                Ok(Value::String(v.to_owned()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
                Ok(Value::String(v))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
                Ok(Value::Integer(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
                i64::try_from(v)
                    .map(Value::Integer)
                    .map_err(|_| E::custom(format!("integer out of range: {v}")))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
                Ok(Value::Float(v))
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
                Ok(Value::Boolean(v))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
                Err(E::custom("TOML has no null"))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
                let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(v) = seq.next_element()? {
                    out.push(v);
                }
                Ok(Value::Array(out))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(k) = map.next_key::<String>()? {
                    if k == DATETIME_KEY {
                        let s: String = map.next_value()?;
                        let dt = parse_datetime(&s).ok_or_else(|| {
                            de::Error::custom(format!("not a TOML datetime: {s}"))
                        })?;
                        return Ok(Value::Datetime(dt));
                    }
                    let v: Value = map.next_value()?;
                    entries.push((k, v));
                }
                Ok(Value::Table(Table::from_entries(entries)))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

impl<'de> de::Deserialize<'de> for Table {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Table(t) => Ok(t),
            other => Err(de::Error::custom(format!(
                "expected a table, found {}",
                type_name(&other)
            ))),
        }
    }
}

impl<'de> de::Deserialize<'de> for Datetime {
    /// Accepts the `toml`-crate private map (the interchange form);
    /// a bare string is NOT a datetime, matching the `toml` crate.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct DtVisitor;

        impl<'de> de::Visitor<'de> for DtVisitor {
            type Value = Datetime;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "a TOML datetime (the {{\"{DATETIME_KEY}\": ...}} map)")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Datetime, A::Error> {
                let mut s: Option<String> = None;
                while let Some(k) = map.next_key::<String>()? {
                    if k == DATETIME_KEY {
                        if s.is_some() {
                            return Err(de::Error::duplicate_field(DATETIME_KEY));
                        }
                        s = Some(map.next_value()?);
                    } else {
                        map.next_value::<de::IgnoredAny>()?;
                    }
                }
                let s = s.ok_or_else(|| de::Error::missing_field(DATETIME_KEY))?;
                parse_datetime(&s)
                    .ok_or_else(|| de::Error::custom(format!("not a TOML datetime: {s}")))
            }
        }

        deserializer.deserialize_any(DtVisitor)
    }
}

/// Parse the four TOML datetime shapes out of their string forms.
fn parse_datetime(s: &str) -> Option<Datetime> {
    fn num(s: &[u8], from: usize, len: usize) -> Option<u32> {
        s.get(from..from + len)
            .and_then(|sl| std::str::from_utf8(sl).ok())
            .and_then(|t| t.parse().ok())
    }
    let b = s.as_bytes();

    // date[Thh:mm:ss[.frac][offset]] | hh:mm:ss[.frac]
    let (year, month, day, mut rest) = if b.len() >= 10 && b[4] == b'-' {
        (
            num(b, 0, 4)? as i32,
            num(b, 5, 2)? as u8,
            num(b, 8, 2)? as u8,
            &b[10..],
        )
    } else {
        (0, 0, 0, b)
    };

    let (hour, minute, second, frac, tail) = if rest.is_empty() {
        return Some(Datetime {
            kind: DatetimeKind::Date,
            year,
            month,
            day,
            hour: 0,
            minute: 0,
            second: 0,
            nanosecond: 0,
            offset_seconds: None,
        });
    } else {
        // the separator: 'T'/'t' per TOML, and a space is accepted by
        // the 1.1 draft's readers
        if matches!(rest[0], b'T' | b't' | b' ') {
            rest = &rest[1..];
        }
        if rest.len() < 8 || rest[2] != b':' {
            return None;
        }
        let sec = if rest.len() >= 8 { num(rest, 6, 2)? } else { 0 };
        let (frac, tail) = if rest.len() > 8 && rest[8] == b'.' {
            let end = rest[9..]
                .iter()
                .position(|c| !c.is_ascii_digit())
                .map(|p| 9 + p)
                .unwrap_or(rest.len());
            let digits = &rest[9..end];
            let mut ns: u32 = std::str::from_utf8(digits).ok()?.parse().unwrap_or(0);
            for _ in digits.len()..9 {
                ns *= 10;
            }
            (ns, &rest[end..])
        } else {
            (0, &rest[8..])
        };
        (
            num(rest, 0, 2)? as u8,
            num(rest, 3, 2)? as u8,
            sec as u8,
            frac,
            tail,
        )
    };

    if tail.is_empty() {
        if year == 0 && month == 0 && day == 0 && b[4] != b'-' {
            return Some(Datetime {
                kind: DatetimeKind::Time,
                year: 0,
                month: 0,
                day: 0,
                hour,
                minute,
                second,
                nanosecond: frac,
                offset_seconds: None,
            });
        }
        return Some(Datetime {
            kind: DatetimeKind::LocalDateTime,
            year,
            month,
            day,
            hour,
            minute,
            second,
            nanosecond: frac,
            offset_seconds: None,
        });
    }

    // offset: Z | +HH:MM | -HH:MM
    let offset = match tail[0] {
        b'Z' | b'z' => 0,
        b'+' | b'-' => {
            if tail.len() != 6 || tail[3] != b':' {
                return None;
            }
            let sign = if tail[0] == b'-' { -1 } else { 1 };
            let h = num(tail, 1, 2)? as i32;
            let m = num(tail, 4, 2)? as i32;
            sign * (h * 3600 + m * 60)
        }
        _ => return None,
    };
    Some(Datetime {
        kind: DatetimeKind::Offset,
        year,
        month,
        day,
        hour,
        minute,
        second,
        nanosecond: frac,
        offset_seconds: Some(offset),
    })
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "a string",
        Value::Integer(_) => "an integer",
        Value::Float(_) => "a float",
        Value::Boolean(_) => "a boolean",
        Value::Datetime(_) => "a datetime",
        Value::Array(_) => "an array",
        Value::Table(_) => "a table",
    }
}

// -----------------------------------------------------------------
// Value as a Deserializer (serde -> walk the parsed tree)
// -----------------------------------------------------------------

struct ValueDeserializer {
    value: Value,
}

impl<'de> de::Deserializer<'de> for ValueDeserializer {
    type Error = SerdeError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Value::String(s) => visitor.visit_string(s),
            Value::Integer(i) => visitor.visit_i64(i),
            Value::Float(x) => visitor.visit_f64(x),
            Value::Boolean(b) => visitor.visit_bool(b),
            Value::Datetime(dt) => visitor.visit_map(DatetimeMapAccess {
                dt: Some(dt),
                delivered: false,
            }),
            Value::Array(a) => visitor.visit_seq(SeqDeserializer {
                items: a.into_iter(),
            }),
            Value::Table(t) => visitor.visit_map(TableDeserializer {
                entries: t.into_entries().into_iter(),
                next_value: None,
            }),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        let (variant, value) = match self.value {
            Value::Table(t) => {
                let mut entries = t.into_entries().into_iter();
                match (entries.next(), entries.next()) {
                    (Some((k, v)), None) => (k, Some(v)),
                    _ => return Err(SerdeError::custom("enum table must have exactly one entry")),
                }
            }
            Value::String(s) => (s, None),
            other => {
                return Err(SerdeError::custom(format!(
                    "expected an enum, found {}",
                    type_name(&other)
                )))
            }
        };
        visitor.visit_enum(EnumDeserializer { variant, value })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map
        struct identifier ignored_any
    }
}

struct SeqDeserializer {
    items: std::vec::IntoIter<Value>,
}

impl<'de> de::SeqAccess<'de> for SeqDeserializer {
    type Error = SerdeError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.items.next() {
            Some(v) => seed.deserialize(ValueDeserializer { value: v }).map(Some),
            None => Ok(None),
        }
    }
}

struct TableDeserializer {
    entries: std::vec::IntoIter<(String, Value)>,
    next_value: Option<Value>,
}

impl<'de> de::MapAccess<'de> for TableDeserializer {
    type Error = SerdeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.entries.next() {
            Some((k, v)) => {
                self.next_value = Some(v);
                seed.deserialize(k.into_deserializer()).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let v = self
            .next_value
            .take()
            .expect("next_value_seed called before next_key_seed");
        seed.deserialize(ValueDeserializer { value: v })
            .map_err(|e| {
                // the toml-crate ergonomics: field-level context on
                // mismatched types
                SerdeError::Custom(format!("{e}"))
            })
    }
}

struct EnumDeserializer {
    variant: String,
    value: Option<Value>,
}

impl<'de> de::EnumAccess<'de> for EnumDeserializer {
    type Error = SerdeError;
    type Variant = VariantDeserializer;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let name = self.variant.clone();
        let de = de::value::StrDeserializer::<SerdeError>::new(name.as_str());
        let variant = seed.deserialize(de)?;
        Ok((variant, VariantDeserializer { value: self.value }))
    }
}

struct VariantDeserializer {
    value: Option<Value>,
}

impl<'de> de::VariantAccess<'de> for VariantDeserializer {
    type Error = SerdeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value {
            None => Ok(()),
            Some(v) => de::Deserialize::deserialize(ValueDeserializer { value: v }),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.value {
            Some(v) => seed.deserialize(ValueDeserializer { value: v }),
            None => Err(SerdeError::custom(
                "expected a newtype variant with a value",
            )),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Some(Value::Array(a)) => visitor.visit_seq(SeqDeserializer {
                items: a.into_iter(),
            }),
            Some(other) => Err(SerdeError::custom(format!(
                "expected a tuple variant, found {}",
                type_name(&other)
            ))),
            None => Err(SerdeError::custom("expected a tuple variant with a value")),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Some(Value::Table(t)) => visitor.visit_map(TableDeserializer {
                entries: t.into_entries().into_iter(),
                next_value: None,
            }),
            Some(other) => Err(SerdeError::custom(format!(
                "expected a struct variant, found {}",
                type_name(&other)
            ))),
            None => Err(SerdeError::custom("expected a struct variant with a value")),
        }
    }
}

/// The single-entry private-map view of a datetime, handed to a
/// visitor through `deserialize_any` (the Datetime Deserialize impl
/// consumes it via its own visitor).
struct DatetimeMapAccess {
    dt: Option<Datetime>,
    delivered: bool,
}

impl<'de> de::MapAccess<'de> for DatetimeMapAccess {
    type Error = SerdeError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.delivered {
            return Ok(None);
        }
        self.delivered = true;
        seed.deserialize(DATETIME_KEY.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let text = self.dt.as_ref().expect("key without value").to_string();
        seed.deserialize(text.into_deserializer())
    }
}

// -----------------------------------------------------------------
// ValueSerializer (serde -> Value, then dump)
// -----------------------------------------------------------------

struct ValueSerializer;

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = SerdeError;
    type SerializeSeq = SerializeSeqValue;
    type SerializeTuple = SerializeSeqValue;
    type SerializeTupleStruct = SerializeSeqValue;
    type SerializeTupleVariant = SerializeSeqValue;
    type SerializeMap = SerializeMapValue;
    type SerializeStruct = SerializeMapValue;
    type SerializeStructVariant = SerializeMapValue;

    fn serialize_bool(self, v: bool) -> Result<Value, Self::Error> {
        Ok(Value::Boolean(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Value, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i16(self, v: i16) -> Result<Value, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i32(self, v: i32) -> Result<Value, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_i64(self, v: i64) -> Result<Value, Self::Error> {
        Ok(Value::Integer(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Value, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_u16(self, v: u16) -> Result<Value, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_u32(self, v: u32) -> Result<Value, Self::Error> {
        self.serialize_i64(i64::from(v))
    }
    fn serialize_u64(self, v: u64) -> Result<Value, Self::Error> {
        i64::try_from(v)
            .map(Value::Integer)
            .map_err(|_| SerdeError::custom(format!("integer out of range: {v}")))
    }
    fn serialize_f32(self, v: f32) -> Result<Value, Self::Error> {
        self.serialize_f64(f64::from(v))
    }
    fn serialize_f64(self, v: f64) -> Result<Value, Self::Error> {
        Ok(Value::Float(v))
    }
    fn serialize_char(self, v: char) -> Result<Value, Self::Error> {
        self.serialize_str(&v.to_string())
    }
    fn serialize_str(self, v: &str) -> Result<Value, Self::Error> {
        Ok(Value::String(v.to_owned()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Value, Self::Error> {
        Ok(Value::Array(
            v.iter().map(|b| Value::Integer(i64::from(*b))).collect(),
        ))
    }

    fn serialize_none(self) -> Result<Value, Self::Error> {
        Err(SerdeError::custom(
            "TOML has no null: skip the field with #[serde(skip_serializing_if = \"Option::is_none\")]",
        ))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Value, Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Self::Error> {
        self.serialize_none()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<Value, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        let mut map = SerializeMapValue::new();
        map.serialize_entry(variant, &value.serialize(ValueSerializer)?)?;
        map.end_map_value()
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeSeqValue {
            items: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _idx: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(SerializeMapValue::new())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(SerializeMapValue::new())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _idx: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(SerializeMapValue::new())
    }
}

struct SerializeSeqValue {
    items: Vec<Value>,
}

impl ser::SerializeSeq for SerializeSeqValue {
    type Ok = Value;
    type Error = SerdeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        Ok(Value::Array(self.items))
    }
}

impl ser::SerializeTuple for SerializeSeqValue {
    type Ok = Value;
    type Error = SerdeError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SerializeSeqValue {
    type Ok = Value;
    type Error = SerdeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for SerializeSeqValue {
    type Ok = Value;
    type Error = SerdeError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

struct SerializeMapValue {
    entries: Vec<(String, Value)>,
    pending_key: Option<String>,
}

impl SerializeMapValue {
    fn new() -> Self {
        SerializeMapValue {
            entries: Vec::new(),
            pending_key: None,
        }
    }

    fn end_map_value(self) -> Result<Value, SerdeError> {
        // the Datetime convention arrives here as the one-entry
        // private map: reconstruct it so TOML output renders a real
        // datetime (the toml crate's own to_string does the same)
        if let [(k, Value::String(text))] = self.entries.as_slice() {
            if k == DATETIME_KEY {
                if let Some(dt) = parse_datetime(text) {
                    return Ok(Value::Datetime(dt));
                }
            }
        }
        Ok(Value::Table(Table::from_entries(self.entries)))
    }
}

impl ser::SerializeMap for SerializeMapValue {
    type Ok = Value;
    type Error = SerdeError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        match key.serialize(ValueSerializer)? {
            Value::String(k) => {
                self.pending_key = Some(k);
                Ok(())
            }
            other => Err(SerdeError::custom(format!(
                "TOML keys must be strings, found {}",
                type_name(&other)
            ))),
        }
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        let key = self
            .pending_key
            .take()
            .expect("serialize_value called before serialize_key");
        self.entries.push((key, value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        self.end_map_value()
    }
}

impl ser::SerializeStruct for SerializeMapValue {
    type Ok = Value;
    type Error = SerdeError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        self.entries
            .push((key.to_owned(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Self::Error> {
        self.end_map_value()
    }
}

impl ser::SerializeStructVariant for SerializeMapValue {
    type Ok = Value;
    type Error = SerdeError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeStruct::serialize_field(self, key, value)
    }

    fn end(self) -> Result<Value, Self::Error> {
        self.end_map_value()
    }
}
