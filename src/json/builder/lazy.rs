//! Path-masked [`LazyJsonValueBuilder`]: materialize only what a [`Need`] requires.
//!
//! Correctness-first: unused object keys and array slots past the needed index
//! are still lexed via `skip_value` (nested containers close properly). Trailing
//! junk after the root value is accepted. The unchanged JQL engine must see a
//! DOM that yields the same query results as an eager parse for the same script.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::json::{
    error::JsonError,
    hook::{JsonAtom, ParseDecision, ParserHookable},
    value::{JsonObject, JsonValue},
};

/// Shape of the input required by a query (built from the JQL AST before parse).
#[derive(Debug, Clone)]
pub enum Need {
    /// Materialize the full subtree.
    Full,
    /// Selective object fields and/or array indices.
    Selective(SelectiveNeed),
}

#[derive(Debug, Clone, Default)]
pub struct SelectiveNeed {
    pub fields: HashMap<String, Need>,
    pub all_fields: bool,
    pub array_all: Option<Box<Need>>,
    pub indices: BTreeMap<usize, Need>,
}

impl Need {
    pub fn empty() -> Self {
        Need::Selective(SelectiveNeed::default())
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Need::Full)
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Need::Full => false,
            Need::Selective(s) => {
                !s.all_fields
                    && s.fields.is_empty()
                    && s.array_all.is_none()
                    && s.indices.is_empty()
            }
        }
    }

    pub fn field(key: impl Into<String>, child: Need) -> Self {
        let mut s = SelectiveNeed::default();
        s.fields.insert(key.into(), child);
        Need::Selective(s)
    }

    pub fn array_all(child: Need) -> Self {
        let mut s = SelectiveNeed::default();
        s.array_all = Some(Box::new(child));
        Need::Selective(s)
    }

    pub fn array_index(index: usize, child: Need) -> Self {
        let mut s = SelectiveNeed::default();
        s.indices.insert(index, child);
        Need::Selective(s)
    }
}

impl SelectiveNeed {
    fn is_empty(&self) -> bool {
        !self.all_fields
            && self.fields.is_empty()
            && self.array_all.is_none()
            && self.indices.is_empty()
    }
}

pub fn merge_need(a: Need, b: Need) -> Need {
    match (a, b) {
        (Need::Full, _) | (_, Need::Full) => Need::Full,
        (Need::Selective(a), Need::Selective(b)) => {
            if a.is_empty() {
                return Need::Selective(b);
            }
            if b.is_empty() {
                return Need::Selective(a);
            }
            if a.all_fields || b.all_fields {
                return Need::Full;
            }
            let mut out = a;
            for (k, bv) in b.fields {
                out.fields
                    .entry(k)
                    .and_modify(|av| {
                        *av = merge_need(std::mem::replace(av, Need::empty()), bv.clone())
                    })
                    .or_insert(bv);
            }
            out.array_all = match (out.array_all.take(), b.array_all) {
                (Some(x), Some(y)) => Some(Box::new(merge_need(*x, *y))),
                (Some(x), None) => Some(x),
                (None, Some(y)) => Some(y),
                (None, None) => None,
            };
            for (i, bv) in b.indices {
                out.indices
                    .entry(i)
                    .and_modify(|av| {
                        *av = merge_need(std::mem::replace(av, Need::empty()), bv.clone())
                    })
                    .or_insert(bv);
            }
            Need::Selective(out)
        }
    }
}

#[derive(Debug)]
enum Frame {
    Full,
    Object {
        fields: HashMap<String, Need>,
        pending: HashSet<String>,
    },
    Array {
        array_all: Option<Need>,
        indices: BTreeMap<usize, Need>,
        /// Inclusive max index that must be visited (Keep or null-pad).
        max_index: Option<usize>,
        /// When true, later elements are skipped without null padding.
        past_needed: bool,
    },
}

/// Eager-compatible [`ParserHookable`] that only materializes paths in `need`.
#[derive(Debug)]
pub struct LazyJsonValueBuilder {
    root: Need,
    stack: Vec<Frame>,
    /// Need for the value about to be parsed after a Keep decision.
    pending_child: Option<Need>,
}

impl LazyJsonValueBuilder {
    pub fn new(need: Need) -> Self {
        Self {
            root: need,
            stack: Vec::new(),
            pending_child: None,
        }
    }

    fn take_descend_need(&mut self) -> Need {
        if let Some(n) = self.pending_child.take() {
            return n;
        }
        if self.stack.is_empty() {
            return self.root.clone();
        }
        Need::Full
    }

    fn push_frame_for(&mut self, need: Need) {
        match need {
            Need::Full => self.stack.push(Frame::Full),
            Need::Selective(s) if s.all_fields => self.stack.push(Frame::Full),
            Need::Selective(s) => {
                if s.array_all.is_some() || !s.indices.is_empty() {
                    let max_index = s.indices.keys().next_back().copied();
                    self.stack.push(Frame::Array {
                        array_all: s.array_all.map(|n| *n),
                        indices: s.indices,
                        max_index,
                        past_needed: false,
                    });
                } else {
                    let pending: HashSet<String> = s.fields.keys().cloned().collect();
                    self.stack.push(Frame::Object {
                        fields: s.fields,
                        pending,
                    });
                }
            }
        }
    }
}

impl<'a> ParserHookable<'a> for LazyJsonValueBuilder {
    type Value = JsonValue<'a>;
    type Array = Vec<JsonValue<'a>>;
    type Object = JsonObject<'a>;
    type Error = JsonError;

    fn accept_trailing_junk(&self) -> bool {
        true
    }

    fn on_atom(&mut self, atom: JsonAtom<'a>) -> Result<Self::Value, Self::Error> {
        // Scalar where a container child need was reserved — drop it.
        self.pending_child = None;
        Ok(match atom {
            JsonAtom::Null => JsonValue::null(),
            JsonAtom::Bool(v) => JsonValue::boolean(v),
            JsonAtom::Number(n) => JsonValue::Number(n),
            JsonAtom::String(s) => JsonValue::string(s),
        })
    }

    fn on_begin_array(&mut self) -> Result<Self::Array, Self::Error> {
        let mut need = self.take_descend_need();
        // Object-field need at an array site → each element is that object shape.
        if let Need::Selective(s) = &need
            && s.array_all.is_none()
            && s.indices.is_empty()
            && !s.fields.is_empty()
            && !s.all_fields
        {
            need = Need::array_all(need);
        }
        self.push_frame_for(need);
        Ok(Vec::new())
    }

    fn want_array_element(&mut self, index: usize) -> Result<ParseDecision, Self::Error> {
        let Some(frame) = self.stack.last_mut() else {
            self.pending_child = Some(Need::Full);
            return Ok(ParseDecision::Keep);
        };
        match frame {
            Frame::Full => {
                self.pending_child = Some(Need::Full);
                Ok(ParseDecision::Keep)
            }
            Frame::Object { .. } => {
                self.pending_child = Some(Need::Full);
                Ok(ParseDecision::Keep)
            }
            Frame::Array {
                array_all,
                indices,
                max_index,
                past_needed,
            } => {
                if *past_needed {
                    self.pending_child = None;
                    return Ok(ParseDecision::Skip);
                }
                if let Some(all) = array_all {
                    let mut child = all.clone();
                    if let Some(idx_need) = indices.get(&index) {
                        child = merge_need(child, idx_need.clone());
                    }
                    self.pending_child = Some(child);
                    return Ok(ParseDecision::Keep);
                }
                if let Some(max) = *max_index {
                    if index > max {
                        *past_needed = true;
                        self.pending_child = None;
                        return Ok(ParseDecision::Skip);
                    }
                    if let Some(child) = indices.get(&index) {
                        self.pending_child = Some(child.clone());
                        Ok(ParseDecision::Keep)
                    } else {
                        // Surplus earlier slot: skip materialize, null-pad.
                        self.pending_child = None;
                        Ok(ParseDecision::Skip)
                    }
                } else {
                    *past_needed = true;
                    self.pending_child = None;
                    Ok(ParseDecision::Skip)
                }
            }
        }
    }

    fn on_array_element(
        &mut self,
        array: &mut Self::Array,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        array.push(value);
        Ok(())
    }

    fn on_array_element_skipped(
        &mut self,
        array: &mut Self::Array,
        _index: usize,
    ) -> Result<(), Self::Error> {
        let pad = matches!(
            self.stack.last(),
            Some(Frame::Array {
                past_needed: false,
                ..
            })
        );
        if pad {
            array.push(JsonValue::Null);
        }
        Ok(())
    }

    fn on_end_array(&mut self, array: Self::Array) -> Result<Self::Value, Self::Error> {
        self.stack.pop();
        Ok(JsonValue::array(array))
    }

    fn on_begin_object(&mut self) -> Result<Self::Object, Self::Error> {
        let need = self.take_descend_need();
        self.push_frame_for(need);
        Ok(JsonObject::new())
    }

    fn want_object_key(&mut self, key: &str) -> Result<ParseDecision, Self::Error> {
        let Some(frame) = self.stack.last_mut() else {
            self.pending_child = Some(Need::Full);
            return Ok(ParseDecision::Keep);
        };
        match frame {
            Frame::Full => {
                self.pending_child = Some(Need::Full);
                Ok(ParseDecision::Keep)
            }
            Frame::Array { .. } => {
                self.pending_child = Some(Need::Full);
                Ok(ParseDecision::Keep)
            }
            Frame::Object { fields, pending } => {
                if pending.is_empty() && !fields.is_empty() {
                    // All required keys collected — skip remaining keys.
                    self.pending_child = None;
                    return Ok(ParseDecision::Skip);
                }
                if fields.is_empty() {
                    self.pending_child = None;
                    return Ok(ParseDecision::Skip);
                }
                if let Some(child) = fields.get(key) {
                    self.pending_child = Some(child.clone());
                    Ok(ParseDecision::Keep)
                } else {
                    self.pending_child = None;
                    Ok(ParseDecision::Skip)
                }
            }
        }
    }

    fn on_object_entry(
        &mut self,
        object: &mut Self::Object,
        key: Cow<'a, str>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        if let Some(Frame::Object { pending, .. }) = self.stack.last_mut() {
            pending.remove(key.as_ref());
        }
        object.push(key, value);
        Ok(())
    }

    fn on_end_object(&mut self, object: Self::Object) -> Result<Self::Value, Self::Error> {
        self.stack.pop();
        Ok(JsonValue::Object(object))
    }
}
