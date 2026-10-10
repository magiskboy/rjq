use std::{fmt::Debug, str::FromStr};

use crate::jql::error::{JqlError, JqlErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertySelection {
    Properties(Vec<String>),
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayIndex {
    Range { start: usize, end: usize },
    Index(usize),
    MultipleIndex(Vec<usize>),
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Access {
    IndexAccess {
        index: ArrayIndex,
        properties: PropertySelection,
    },
    PropertyAccess {
        property: String,
        properties: PropertySelection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub access: Vec<Access>,
}

#[derive(Debug, Clone)]
enum ParseReferenceState {
    StartProperty,
    InProperty,
    StartArrayIndex,
    InArrayIndex,
    InArraySlice,
    InArraySliceColon,
    InMultipleIndex,
    InMultipleIndexComma,
    CloseArrayIndex,
    StartPropertySelection,
    InPropertySelection,
    InPropertyComma,
    ClosePropertySelection,
    InvalidCharacter,
}

impl Reference {
    fn accept_state(
        state: &ParseReferenceState,
        start: usize,
        end: usize,
        source: &str,
        last_access: &Access,
    ) -> Result<Access, JqlError> {
        match state {
            ParseReferenceState::InProperty => {
                let property = source
                    .get(start..end)
                    .ok_or_else(|| Self::invalid_access(source))?
                    .to_string();
                Ok(Access::PropertyAccess {
                    property,
                    properties: PropertySelection::All,
                })
            }

            ParseReferenceState::CloseArrayIndex => {
                let s = source
                    .get(start..end - 1)
                    .ok_or_else(|| Self::invalid_access(source))?;

                if s.is_empty() {
                    return Ok(Access::IndexAccess {
                        index: ArrayIndex::All,
                        properties: PropertySelection::All,
                    });
                }

                if let Ok(index) = Self::parse_usize(s, source) {
                    return Ok(Access::IndexAccess {
                        index: ArrayIndex::Index(index),
                        properties: PropertySelection::All,
                    });
                }

                let parts: Vec<&str> = s.split(':').collect();
                if parts.len() == 2 {
                    let start = Self::parse_usize(parts[0], source)?;
                    let end = Self::parse_usize(parts[1], source)?;
                    return Ok(Access::IndexAccess {
                        index: ArrayIndex::Range { start, end },
                        properties: PropertySelection::All,
                    });
                }

                let indices = s
                    .split(',')
                    .map(|part| Self::parse_usize(part, source))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Access::IndexAccess {
                    index: ArrayIndex::MultipleIndex(indices),
                    properties: PropertySelection::All,
                })
            }

            ParseReferenceState::ClosePropertySelection => {
                let s = source
                    .get(start..end - 1)
                    .ok_or_else(|| Self::invalid_access(source))?;
                if let Some(lbrace) = s.find('{') {
                    let keys: Vec<String> = s
                        .get(lbrace + 1..)
                        .ok_or_else(|| Self::invalid_access(source))?
                        .split(",")
                        .map(|x| x.to_string())
                        .filter(|x| !x.is_empty())
                        .collect();

                    let selected_props = if keys.is_empty() {
                        PropertySelection::All
                    } else {
                        PropertySelection::Properties(keys)
                    };
                    return Ok(match last_access {
                        Access::IndexAccess {
                            index,
                            properties: _,
                        } => Access::IndexAccess {
                            index: index.clone(),
                            properties: selected_props,
                        },
                        Access::PropertyAccess {
                            property,
                            properties: _,
                        } => Access::PropertyAccess {
                            property: property.clone(),
                            properties: selected_props,
                        },
                    });
                }
                Err(Self::invalid_access(source))
            }
            _ => Err(Self::invalid_access(source)),
        }
    }

    fn transition_table(state: &ParseReferenceState, c: char) -> Option<ParseReferenceState> {
        let next_state = match state {
            ParseReferenceState::StartProperty => match c {
                'a'..='z' | 'A'..='Z' => ParseReferenceState::InProperty,
                '{' => ParseReferenceState::StartPropertySelection,
                '[' => ParseReferenceState::StartArrayIndex,
                '.' => ParseReferenceState::StartProperty,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InProperty => match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' => ParseReferenceState::InProperty,
                '.' => ParseReferenceState::StartProperty,
                '[' => ParseReferenceState::StartArrayIndex,
                '{' => ParseReferenceState::StartPropertySelection,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::StartArrayIndex => match c {
                '0'..='9' => ParseReferenceState::InArrayIndex,
                ']' => ParseReferenceState::CloseArrayIndex,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InArrayIndex => match c {
                '0'..='9' => ParseReferenceState::InArrayIndex,
                ']' => ParseReferenceState::CloseArrayIndex,
                ',' => ParseReferenceState::InMultipleIndexComma,
                ':' => ParseReferenceState::InArraySliceColon,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InMultipleIndexComma => match c {
                '0'..='9' => ParseReferenceState::InMultipleIndex,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InMultipleIndex => match c {
                '0'..='9' => ParseReferenceState::InMultipleIndex,
                ',' => ParseReferenceState::InMultipleIndexComma,
                ']' => ParseReferenceState::CloseArrayIndex,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InArraySliceColon => match c {
                '0'..='9' => ParseReferenceState::InArraySlice,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InArraySlice => match c {
                '0'..='9' => ParseReferenceState::InArraySlice,
                ']' => ParseReferenceState::CloseArrayIndex,
                _ => ParseReferenceState::InvalidCharacter,
            },

            ParseReferenceState::StartPropertySelection => match c {
                'a'..='z' | 'A'..='Z' => ParseReferenceState::InPropertySelection,
                ',' => ParseReferenceState::InPropertyComma,
                '}' => ParseReferenceState::ClosePropertySelection,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InPropertySelection => match c {
                'a'..='z' | 'A'..='Z' => ParseReferenceState::InPropertySelection,
                ',' => ParseReferenceState::InPropertyComma,
                '}' => ParseReferenceState::ClosePropertySelection,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::InPropertyComma => match c {
                'a'..='z' | 'A'..='Z' => ParseReferenceState::InPropertySelection,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::CloseArrayIndex => match c {
                '.' => ParseReferenceState::StartProperty,
                '[' => ParseReferenceState::StartArrayIndex,
                '{' => ParseReferenceState::StartPropertySelection,
                _ => ParseReferenceState::InvalidCharacter,
            },
            ParseReferenceState::ClosePropertySelection => match c {
                '.' => ParseReferenceState::StartProperty,
                _ => ParseReferenceState::InvalidCharacter,
            },

            _ => ParseReferenceState::InvalidCharacter,
        };

        match next_state {
            ParseReferenceState::InvalidCharacter => None,
            o => Some(o),
        }
    }

    fn invalid_access(path: &str) -> JqlError {
        JqlError::without_span(JqlErrorKind::InvalidAccess {
            path: path.to_string(),
        })
    }

    fn parse_usize(raw: &str, path: &str) -> Result<usize, JqlError> {
        raw.parse::<usize>()
            .map_err(|_| Self::invalid_access(path))
    }
}

impl FromStr for Reference {
    type Err = JqlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('.') {
            return Err(Self::invalid_access(s));
        }

        // Root reference: identity over the current document.
        if s == "." {
            return Ok(Self {
                access: vec![Access::PropertyAccess {
                    property: String::new(),
                    properties: PropertySelection::All,
                }],
            });
        }

        let mut access: Vec<Access> = vec![Access::PropertyAccess {
            property: String::new(),
            properties: PropertySelection::All,
        }];
        let mut idx: usize = 1;
        let mut state = ParseReferenceState::StartProperty;
        let mut prop = access[0].clone();
        let mut start: usize = 1;

        while idx < s.len() {
            let ch = s.chars().nth(idx).ok_or_else(|| Self::invalid_access(s))?;
            let Some(new_state) = Self::transition_table(&state, ch) else {
                return Err(Self::invalid_access(s));
            };

            state = new_state;
            if let Ok(v) = Self::accept_state(&state, start, idx + ch.len_utf8(), s, &prop) {
                prop = v;
            }

            if matches!(
                state,
                ParseReferenceState::StartProperty | ParseReferenceState::StartArrayIndex
            ) {
                access.push(prop);
                start = idx + ch.len_utf8();

                prop = match state {
                    ParseReferenceState::StartProperty => Access::PropertyAccess {
                        property: String::new(),
                        properties: PropertySelection::All,
                    },
                    ParseReferenceState::StartArrayIndex => Access::IndexAccess {
                        index: ArrayIndex::All,
                        properties: PropertySelection::All,
                    },
                    _ => unreachable!(),
                }
            }

            idx += ch.len_utf8();
        }

        access.push(prop);

        match (access.get(0), access.get(1)) {
            (
                Some(Access::PropertyAccess {
                    property: p1,
                    properties: _,
                }),
                Some(Access::PropertyAccess {
                    property: p2,
                    properties: _,
                }),
            ) => {
                if p1 == p2 && p1.is_empty() {
                    access.remove(0);
                }
            }
            _ => unreachable!(),
        }

        match state {
            ParseReferenceState::InProperty
            | ParseReferenceState::CloseArrayIndex
            | ParseReferenceState::ClosePropertySelection => Ok(Self { access }),
            _ => Err(Self::invalid_access(s)),
        }
    }
}


#[cfg(test)]
#[path = "../../tests/jql/reference.rs"]
mod test;
