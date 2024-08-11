use enum_as_inner::EnumAsInner;
use fxhash::FxHashMap;
use once_cell::sync::OnceCell;
use serde::{de::VariantAccess, Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash, ops::Index, sync::Arc};

use crate::ContainerID;

/// [LoroValue] is used to represents the state of CRDT at a given version.
///
/// This struct is cheap to clone, the time complexity is O(1).
#[derive(Debug, PartialEq, Clone, EnumAsInner, Default)]
pub enum LoroValue {
    #[default]
    Null,
    Bool(bool),
    Double(f64),
    I64(i64),
    // i64?
    Binary(Arc<(Box<[u8]>, OnceCell<u64>)>),
    String(Arc<(String, OnceCell<u64>)>),
    List(Arc<(Vec<LoroValue>, OnceCell<u64>)>),
    // PERF We can use InternalString as key
    Map(Arc<(FxHashMap<String, LoroValue>, OnceCell<u64>)>),
    Container(ContainerID),
}

const MAX_DEPTH: usize = 128;
impl<'a> arbitrary::Arbitrary<'a> for LoroValue {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let value = match u.int_in_range(0..=7).unwrap() {
            0 => LoroValue::Null,
            1 => LoroValue::Bool(u.arbitrary()?),
            2 => LoroValue::Double(u.arbitrary()?),
            3 => LoroValue::I64(u.arbitrary()?),
            4 => LoroValue::Binary(Arc::new((u.arbitrary()?, OnceCell::new()))),
            5 => LoroValue::String(Arc::new((u.arbitrary()?, OnceCell::new()))),
            6 => LoroValue::List(Arc::new((u.arbitrary()?, OnceCell::new()))),
            7 => LoroValue::Map(Arc::new((u.arbitrary()?, OnceCell::new()))),
            _ => unreachable!(),
        };

        if value.get_depth() > MAX_DEPTH {
            Err(arbitrary::Error::IncorrectFormat)
        } else {
            Ok(value)
        }
    }
}

impl LoroValue {
    pub fn get_by_key(&self, key: &str) -> Option<&LoroValue> {
        match self {
            LoroValue::Map(map) => map.0.get(key),
            _ => None,
        }
    }

    pub fn get_by_index(&self, index: usize) -> Option<&LoroValue> {
        match self {
            LoroValue::List(list) => list.0.get(index),
            _ => None,
        }
    }

    pub fn is_false(&self) -> bool {
        match self {
            LoroValue::Bool(b) => !*b,
            _ => false,
        }
    }

    pub fn get_depth(&self) -> usize {
        let mut max_depth = 0;
        let mut value_depth_pairs = vec![(self, 0)];
        while let Some((value, depth)) = value_depth_pairs.pop() {
            match value {
                LoroValue::List(arr) => {
                    for v in arr.0.iter() {
                        value_depth_pairs.push((v, depth + 1));
                    }
                    max_depth = max_depth.max(depth + 1);
                }
                LoroValue::Map(map) => {
                    for (_, v) in map.0.iter() {
                        value_depth_pairs.push((v, depth + 1));
                    }

                    max_depth = max_depth.max(depth + 1);
                }
                _ => {}
            }
        }

        max_depth
    }

    // TODO: add checks for too deep value, and return err if users
    // try to insert such value into a container
    pub fn is_too_deep(&self) -> bool {
        self.get_depth() > MAX_DEPTH
    }
}

impl Index<&str> for LoroValue {
    type Output = LoroValue;

    fn index(&self, index: &str) -> &Self::Output {
        match self {
            LoroValue::Map(map) => map.0.get(index).unwrap_or(&LoroValue::Null),
            _ => &LoroValue::Null,
        }
    }
}

impl Index<usize> for LoroValue {
    type Output = LoroValue;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            LoroValue::List(list) => list.0.get(index).unwrap_or(&LoroValue::Null),
            _ => &LoroValue::Null,
        }
    }
}

impl TryFrom<LoroValue> for bool {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::Bool(v) => Ok(v),
            _ => Err("not a bool"),
        }
    }
}

impl TryFrom<LoroValue> for f64 {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::Double(v) => Ok(v),
            _ => Err("not a double"),
        }
    }
}

impl TryFrom<LoroValue> for i32 {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::I64(v) => Ok(v as i32),
            _ => Err("not a i32"),
        }
    }
}

impl TryFrom<LoroValue> for Arc<Vec<u8>> {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::Binary(v) => Ok(Arc::new(v.0.to_vec())),
            _ => Err("not a binary"),
        }
    }
}

impl TryFrom<LoroValue> for Arc<String> {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::String(v) => Ok(Arc::new(v.0.clone())),
            _ => Err("not a string"),
        }
    }
}

impl TryFrom<LoroValue> for Arc<Vec<LoroValue>> {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::List(v) => Ok(Arc::new(v.0.clone())),
            _ => Err("not a list"),
        }
    }
}

impl TryFrom<LoroValue> for Arc<FxHashMap<String, LoroValue>> {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::Map(v) => Ok(Arc::new(v.0.clone())),
            _ => Err("not a map"),
        }
    }
}

impl TryFrom<LoroValue> for ContainerID {
    type Error = &'static str;

    fn try_from(value: LoroValue) -> Result<Self, Self::Error> {
        match value {
            LoroValue::Container(v) => Ok(v),
            _ => Err("not a container"),
        }
    }
}

impl Hash for LoroValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            LoroValue::Null => {}
            LoroValue::Bool(v) => {
                state.write_u8(*v as u8);
            }
            LoroValue::Double(v) => {
                state.write_u64(v.to_bits());
            }
            LoroValue::I64(v) => {
                state.write_i64(*v);
            }
            LoroValue::Binary(v) => match v.1.get() {
                Some(v) => state.write_u64(*v),
                None => {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    v.0.hash(&mut hasher);
                    let hash = std::hash::Hasher::finish(&hasher);
                    v.1.set(hash).unwrap();
                    state.write_u64(hash);
                }
            },
            LoroValue::String(v) => match v.1.get() {
                Some(v) => state.write_u64(*v),
                None => {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    v.0.hash(&mut hasher);
                    let hash = std::hash::Hasher::finish(&hasher);
                    v.1.set(hash).unwrap();
                    state.write_u64(hash);
                }
            },
            LoroValue::List(v) => match v.1.get() {
                Some(v) => state.write_u64(*v),
                None => {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    v.0.hash(&mut hasher);
                    let hash = std::hash::Hasher::finish(&hasher);
                    v.1.set(hash).unwrap();
                    state.write_u64(hash);
                }
            },
            LoroValue::Map(v) => match v.1.get() {
                Some(v) => state.write_u64(*v),
                None => {
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hasher::write_usize(&mut hasher, v.0.len());
                    for (k, v) in v.0.iter() {
                        k.hash(&mut hasher);
                        v.hash(&mut hasher);
                    }
                    let hash = std::hash::Hasher::finish(&hasher);
                    v.1.set(hash).unwrap();
                    state.write_u64(hash);
                }
            },
            LoroValue::Container(v) => {
                v.hash(state);
            }
        }
    }
}

impl Eq for LoroValue {}

impl<S: Into<String>, M> From<HashMap<S, LoroValue, M>> for LoroValue {
    fn from(map: HashMap<S, LoroValue, M>) -> Self {
        let mut new_map = FxHashMap::default();
        for (k, v) in map {
            new_map.insert(k.into(), v);
        }

        LoroValue::Map(Arc::new((new_map, OnceCell::new())))
    }
}

impl From<Vec<u8>> for LoroValue {
    fn from(vec: Vec<u8>) -> Self {
        LoroValue::Binary(Arc::new((Box::from(vec), OnceCell::new())))
    }
}

impl From<&'_ [u8]> for LoroValue {
    fn from(vec: &[u8]) -> Self {
        LoroValue::Binary(Arc::new((Box::from(vec), OnceCell::new())))
    }
}

impl<const N: usize> From<&'_ [u8; N]> for LoroValue {
    fn from(vec: &[u8; N]) -> Self {
        LoroValue::Binary(Arc::new((Box::from(*vec), OnceCell::new())))
    }
}

impl From<i32> for LoroValue {
    fn from(v: i32) -> Self {
        LoroValue::I64(v as i64)
    }
}

impl From<u32> for LoroValue {
    fn from(v: u32) -> Self {
        LoroValue::I64(v as i64)
    }
}

impl From<i64> for LoroValue {
    fn from(v: i64) -> Self {
        LoroValue::I64(v)
    }
}

impl From<u16> for LoroValue {
    fn from(v: u16) -> Self {
        LoroValue::I64(v as i64)
    }
}

impl From<i16> for LoroValue {
    fn from(v: i16) -> Self {
        LoroValue::I64(v as i64)
    }
}

impl From<f64> for LoroValue {
    fn from(v: f64) -> Self {
        LoroValue::Double(v)
    }
}

impl From<bool> for LoroValue {
    fn from(v: bool) -> Self {
        LoroValue::Bool(v)
    }
}

impl<T: Into<LoroValue>> From<Vec<T>> for LoroValue {
    fn from(value: Vec<T>) -> Self {
        let vec: Vec<LoroValue> = value.into_iter().map(|x| x.into()).collect();
        Self::List(Arc::new((vec, OnceCell::new())))
    }
}

impl From<&str> for LoroValue {
    fn from(v: &str) -> Self {
        LoroValue::String(Arc::new((v.to_string(), OnceCell::new())))
    }
}

impl From<String> for LoroValue {
    fn from(v: String) -> Self {
        LoroValue::String(Arc::new((v, OnceCell::new())))
    }
}

impl<'a> From<&'a [LoroValue]> for LoroValue {
    fn from(v: &'a [LoroValue]) -> Self {
        LoroValue::List(Arc::new((v.to_vec(), OnceCell::new())))
    }
}

impl From<ContainerID> for LoroValue {
    fn from(v: ContainerID) -> Self {
        LoroValue::Container(v)
    }
}

#[cfg(feature = "wasm")]
pub mod wasm {
    use std::sync::Arc;

    use fxhash::FxHashMap;
    use js_sys::{Array, Object, Uint8Array};
    use once_cell::sync::OnceCell;
    use wasm_bindgen::{JsCast, JsValue, __rt::IntoJsResult};

    use crate::{ContainerID, LoroError, LoroValue};

    pub fn convert(value: LoroValue) -> JsValue {
        match value {
            LoroValue::Null => JsValue::NULL,
            LoroValue::Bool(b) => JsValue::from_bool(b),
            LoroValue::Double(f) => JsValue::from_f64(f),
            LoroValue::I64(i) => JsValue::from_f64(i as f64),
            LoroValue::String(s) => JsValue::from_str(&s.0),
            LoroValue::Binary(binary) => {
                let binary = Arc::try_unwrap(binary).unwrap_or_else(|m| (*m).clone());
                let arr = Uint8Array::new_with_length(binary.0.len() as u32);
                for (i, v) in binary.0.into_iter().enumerate() {
                    arr.set_index(i as u32, *v);
                }
                arr.into_js_result().unwrap()
            }
            LoroValue::List(list) => {
                let list = Arc::try_unwrap(list).unwrap_or_else(|m| (*m).clone());
                let arr = Array::new_with_length(list.0.len() as u32);
                for (i, v) in list.0.into_iter().enumerate() {
                    arr.set(i as u32, convert(v));
                }
                arr.into_js_result().unwrap()
            }
            LoroValue::Map(m) => {
                let m = Arc::try_unwrap(m).unwrap_or_else(|m| (*m).clone());
                let map = Object::new();
                for (k, v) in m.0.into_iter() {
                    let str: &str = &k;
                    js_sys::Reflect::set(&map, &JsValue::from_str(str), &convert(v)).unwrap();
                }

                map.into_js_result().unwrap()
            }
            LoroValue::Container(container_id) => JsValue::from(&container_id),
        }
    }

    impl From<LoroValue> for JsValue {
        fn from(value: LoroValue) -> Self {
            convert(value)
        }
    }

    impl From<JsValue> for LoroValue {
        fn from(js_value: JsValue) -> Self {
            if js_value.is_null() || js_value.is_undefined() {
                LoroValue::Null
            } else if js_value.as_bool().is_some() {
                LoroValue::Bool(js_value.as_bool().unwrap())
            } else if js_value.as_f64().is_some() {
                let num = js_value.as_f64().unwrap();
                if num.fract() == 0.0 && num <= i64::MAX as f64 && num >= i64::MIN as f64 {
                    LoroValue::I64(num as i64)
                } else {
                    LoroValue::Double(num)
                }
            } else if js_value.is_string() {
                LoroValue::String(Arc::new((js_value.as_string().unwrap(), OnceCell::new())))
            } else if js_value.has_type::<Array>() {
                let array = js_value.unchecked_into::<Array>();
                let mut list = Vec::new();
                for i in 0..array.length() {
                    list.push(LoroValue::from(array.get(i)));
                }

                LoroValue::List(Arc::new((list, OnceCell::new())))
            } else if js_value.is_instance_of::<Uint8Array>() {
                let array = js_value.unchecked_into::<Uint8Array>();
                let mut binary = Vec::new();
                for i in 0..array.length() {
                    binary.push(array.get_index(i));
                }

                LoroValue::Binary(Arc::new((Box::from(binary), OnceCell::new())))
            } else if js_value.is_object() {
                let object = js_value.unchecked_into::<Object>();
                let mut map = FxHashMap::default();
                for key in js_sys::Reflect::own_keys(&object).unwrap().iter() {
                    let key = key.as_string().unwrap();
                    map.insert(
                        key.clone(),
                        LoroValue::from(js_sys::Reflect::get(&object, &key.into()).unwrap()),
                    );
                }

                LoroValue::Map(Arc::new((map, OnceCell::new())))
            } else {
                panic!("Fail to convert JsValue {:?} to LoroValue ", js_value)
            }
        }
    }

    impl From<&ContainerID> for JsValue {
        fn from(id: &ContainerID) -> Self {
            JsValue::from_str(id.to_string().as_str())
        }
    }

    impl TryFrom<JsValue> for ContainerID {
        type Error = LoroError;

        fn try_from(value: JsValue) -> Result<Self, Self::Error> {
            if !value.is_string() {
                return Err(LoroError::DecodeError(
                    "Given ContainerId is not string".into(),
                ));
            }

            let s = value.as_string().unwrap();
            ContainerID::try_from(s.as_str()).map_err(|_| {
                LoroError::DecodeError(
                    format!("Given ContainerId is not a valid ContainerID: {}", s).into(),
                )
            })
        }
    }
}

const LORO_CONTAINER_ID_PREFIX: &str = "🦜:";

impl Serialize for LoroValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            // json type
            match self {
                LoroValue::Null => serializer.serialize_unit(),
                LoroValue::Bool(b) => serializer.serialize_bool(*b),
                LoroValue::Double(d) => serializer.serialize_f64(*d),
                LoroValue::I64(i) => serializer.serialize_i64(*i),
                LoroValue::String(s) => serializer.serialize_str(&s.0),
                LoroValue::Binary(b) => serializer.collect_seq(b.0.iter()),
                LoroValue::List(l) => serializer.collect_seq(l.0.iter()),
                LoroValue::Map(m) => serializer.collect_map(m.0.iter()),
                LoroValue::Container(id) => {
                    serializer.serialize_str(&format!("{}{}", LORO_CONTAINER_ID_PREFIX, id))
                }
            }
        } else {
            // binary type
            match self {
                LoroValue::Null => serializer.serialize_unit_variant("LoroValue", 0, "Null"),
                LoroValue::Bool(b) => {
                    serializer.serialize_newtype_variant("LoroValue", 1, "Bool", b)
                }
                LoroValue::Double(d) => {
                    serializer.serialize_newtype_variant("LoroValue", 2, "Double", d)
                }
                LoroValue::I64(i) => serializer.serialize_newtype_variant("LoroValue", 3, "I32", i),
                LoroValue::String(s) => {
                    serializer.serialize_newtype_variant("LoroValue", 4, "String", &s.0)
                }

                LoroValue::List(l) => {
                    serializer.serialize_newtype_variant("LoroValue", 5, "List", &l.0)
                }
                LoroValue::Map(m) => {
                    serializer.serialize_newtype_variant("LoroValue", 6, "Map", &m.0)
                }
                LoroValue::Container(id) => {
                    serializer.serialize_newtype_variant("LoroValue", 7, "Container", id)
                }
                LoroValue::Binary(b) => {
                    serializer.serialize_newtype_variant("LoroValue", 8, "Binary", &b.0)
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for LoroValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(LoroValueVisitor)
        } else {
            deserializer.deserialize_enum(
                "LoroValue",
                &[
                    "Null",
                    "Bool",
                    "Double",
                    "I32",
                    "String",
                    "List",
                    "Map",
                    "Container",
                    "Binary",
                ],
                LoroValueEnumVisitor,
            )
        }
    }
}

struct LoroValueVisitor;

impl<'de> serde::de::Visitor<'de> for LoroValueVisitor {
    type Value = LoroValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a LoroValue")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(LoroValue::Null)
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(LoroValue::Bool(v))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(LoroValue::I64(v))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(LoroValue::I64(v as i64))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(LoroValue::Double(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if let Some(id) = v.strip_prefix(LORO_CONTAINER_ID_PREFIX) {
            return Ok(LoroValue::Container(
                ContainerID::try_from(id)
                    .map_err(|_| serde::de::Error::custom("Invalid container id"))?,
            ));
        }
        Ok(LoroValue::String(Arc::new((v.to_owned(), OnceCell::new()))))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if let Some(id) = v.strip_prefix(LORO_CONTAINER_ID_PREFIX) {
            return Ok(LoroValue::Container(
                ContainerID::try_from(id)
                    .map_err(|_| serde::de::Error::custom("Invalid container id"))?,
            ));
        }

        Ok(LoroValue::String(Arc::new((v, OnceCell::new()))))
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let binary = Vec::from_iter(v.iter().copied());
        Ok(LoroValue::Binary(Arc::new((
            Box::from(binary),
            OnceCell::new(),
        ))))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let binary = Vec::from_iter(v.iter().copied());
        Ok(LoroValue::Binary(Arc::new((
            Box::from(binary),
            OnceCell::new(),
        ))))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut list = Vec::new();
        while let Some(value) = seq.next_element()? {
            list.push(value);
        }
        Ok(LoroValue::List(Arc::new((list, OnceCell::new()))))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut ans: FxHashMap<String, _> = FxHashMap::default();
        while let Some((key, value)) = map.next_entry::<String, _>()? {
            ans.insert(key, value);
        }

        Ok(LoroValue::Map(Arc::new((ans, OnceCell::new()))))
    }
}

#[derive(Deserialize)]
enum LoroValueFields {
    Null,
    Bool,
    Double,
    I32,
    String,
    List,
    Map,
    Container,
    Binary,
}

struct LoroValueEnumVisitor;
impl<'de> serde::de::Visitor<'de> for LoroValueEnumVisitor {
    type Value = LoroValue;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a loro value")
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::EnumAccess<'de>,
    {
        match data.variant()? {
            (LoroValueFields::Null, v) => {
                v.unit_variant()?;
                Ok(LoroValue::Null)
            }
            (LoroValueFields::Bool, v) => v.newtype_variant().map(LoroValue::Bool),
            (LoroValueFields::Double, v) => v.newtype_variant().map(LoroValue::Double),
            (LoroValueFields::I32, v) => v.newtype_variant().map(LoroValue::I64),
            (LoroValueFields::String, v) => v
                .newtype_variant()
                .map(|x| LoroValue::String(Arc::new((x, OnceCell::new())))),
            (LoroValueFields::List, v) => v
                .newtype_variant()
                .map(|x| LoroValue::List(Arc::new((x, OnceCell::new())))),
            (LoroValueFields::Map, v) => v
                .newtype_variant()
                .map(|x| LoroValue::Map(Arc::new((x, OnceCell::new())))),
            (LoroValueFields::Container, v) => v.newtype_variant().map(|x| LoroValue::Container(x)),
            (LoroValueFields::Binary, v) => v
                .newtype_variant()
                .map(|x: Vec<u8>| LoroValue::Binary(Arc::new((Box::from(x), OnceCell::new())))),
        }
    }
}

pub fn to_value<T: Into<LoroValue>>(value: T) -> LoroValue {
    value.into()
}
