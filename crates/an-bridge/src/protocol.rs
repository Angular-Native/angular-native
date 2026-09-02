//! The binary protocol between JS and the core.
//!
//! JS does not call into Rust once per mutation: it writes commands into a
//! buffer and hands the whole thing over at the end of the tick. An `*ngFor`
//! over 200 rows is ~1,200 mutations; with individual calls that is 1,200
//! border crossings, and with a buffer it is one.
//!
//! All little-endian. Strings carry a `u32` length and UTF-8 bytes, with no
//! alignment: the decoder reads byte by byte and does not need any.

use an_core::{NodeKind, PropValue, ShadowTree};

pub mod op {
    pub const CREATE_NODE: u8 = 0x01;
    pub const DESTROY_NODE: u8 = 0x02;
    pub const INSERT_CHILD: u8 = 0x03;
    pub const REMOVE_CHILD: u8 = 0x04;
    pub const SET_STYLE: u8 = 0x05;
    pub const SET_PROP_STR: u8 = 0x06;
    pub const SET_PROP_NUM: u8 = 0x07;
    pub const SET_PROP_BOOL: u8 = 0x08;
    pub const SET_PROP_NULL: u8 = 0x09;
    pub const SET_TEXT: u8 = 0x0A;
    pub const SET_LISTENER: u8 = 0x0B;
    pub const SET_ROOT: u8 = 0x0C;
}

#[derive(Debug, PartialEq)]
pub enum ProtocolError {
    /// The buffer ran out halfway through a command.
    Truncated { offset: usize },
    UnknownOpcode { opcode: u8, offset: usize },
    UnknownKind { kind: u8, offset: usize },
    InvalidUtf8 { offset: usize },
    /// The core turned the mutation down (no such node, duplicate id...).
    /// It carries the opcode so the failure says *which* command failed, not
    /// merely that one of them did.
    Tree { opcode: u8, offset: usize, error: an_core::tree::Error },
}

pub fn kind_from_byte(byte: u8) -> Option<NodeKind> {
    Some(match byte {
        0 => NodeKind::View,
        1 => NodeKind::Text,
        2 => NodeKind::RawText,
        3 => NodeKind::Image,
        4 => NodeKind::ScrollView,
        5 => NodeKind::TextInput,
        6 => NodeKind::StackView,
        7 => NodeKind::TabBar,
        8 => NodeKind::Switch,
        9 => NodeKind::Slider,
        10 => NodeKind::ActivityIndicator,
        11 => NodeKind::ProgressBar,
        12 => NodeKind::Button,
        13 => NodeKind::Modal,
        14 => NodeKind::Alert,
        15 => NodeKind::Icon,
        16 => NodeKind::SegmentedControl,
        17 => NodeKind::Stepper,
        18 => NodeKind::SearchBar,
        19 => NodeKind::Picker,
        20 => NodeKind::DatePicker,
        21 => NodeKind::NavigationBar,
        22 => NodeKind::TextEditor,
        23 => NodeKind::WebView,
        24 => NodeKind::MapView,
        25 => NodeKind::VideoView,
        _ => return None,
    })
}

pub fn kind_to_byte(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::View => 0,
        NodeKind::Text => 1,
        NodeKind::RawText => 2,
        NodeKind::Image => 3,
        NodeKind::ScrollView => 4,
        NodeKind::TextInput => 5,
        NodeKind::StackView => 6,
        NodeKind::TabBar => 7,
        NodeKind::Switch => 8,
        NodeKind::Slider => 9,
        NodeKind::ActivityIndicator => 10,
        NodeKind::ProgressBar => 11,
        NodeKind::Button => 12,
        NodeKind::Modal => 13,
        NodeKind::Alert => 14,
        NodeKind::Icon => 15,
        NodeKind::SegmentedControl => 16,
        NodeKind::Stepper => 17,
        NodeKind::SearchBar => 18,
        NodeKind::Picker => 19,
        NodeKind::DatePicker => 20,
        NodeKind::NavigationBar => 21,
        NodeKind::TextEditor => 22,
        NodeKind::WebView => 23,
        NodeKind::MapView => 24,
        NodeKind::VideoView => 25,
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, ProtocolError> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(ProtocolError::Truncated { offset: self.offset })?;
        self.offset += 1;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let end = self.offset + 4;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProtocolError::Truncated { offset: self.offset })?;
        self.offset = end;
        Ok(u32::from_le_bytes(slice.try_into().expect("4 bytes")))
    }

    fn f64(&mut self) -> Result<f64, ProtocolError> {
        let end = self.offset + 8;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProtocolError::Truncated { offset: self.offset })?;
        self.offset = end;
        Ok(f64::from_le_bytes(slice.try_into().expect("8 bytes")))
    }

    fn str(&mut self) -> Result<&'a str, ProtocolError> {
        let start = self.offset;
        let len = self.u32()? as usize;
        let end = self.offset + len;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProtocolError::Truncated { offset: start })?;
        self.offset = end;
        std::str::from_utf8(slice).map_err(|_| ProtocolError::InvalidUtf8 { offset: start })
    }
}

/// Decodes the buffer and applies every command to the tree. Returns how many
/// it applied.
///
/// An error leaves the tree with everything applied up to that point: there is
/// no rollback. That is deliberate — a malformed buffer is a bug on the JS
/// side, not an expected condition, and leaving it half-done puts the failure
/// on the screen where it can be seen.
pub fn apply(bytes: &[u8], tree: &mut ShadowTree) -> Result<usize, ProtocolError> {
    let mut reader = Reader { bytes, offset: 0 };
    let mut applied = 0_usize;

    while reader.offset < bytes.len() {
        let start = reader.offset;
        let opcode = reader.u8()?;
        let result = match opcode {
            op::CREATE_NODE => {
                let id = reader.u32()?;
                let raw = reader.u8()?;
                let kind = kind_from_byte(raw)
                    .ok_or(ProtocolError::UnknownKind { kind: raw, offset: start })?;
                tree.create_node(id, kind)
            }
            op::DESTROY_NODE => tree.destroy_node(reader.u32()?),
            op::INSERT_CHILD => {
                let (parent, child, index) = (reader.u32()?, reader.u32()?, reader.u32()?);
                tree.insert_child(parent, child, index as usize)
            }
            op::REMOVE_CHILD => {
                let (parent, child) = (reader.u32()?, reader.u32()?);
                tree.remove_child(parent, child)
            }
            op::SET_STYLE => {
                let id = reader.u32()?;
                let name = reader.str()?;
                let value = reader.str()?;
                tree.set_style(id, name, value)
            }
            op::SET_PROP_STR => {
                let id = reader.u32()?;
                let key = reader.str()?;
                let value = reader.str()?;
                tree.set_prop(id, key, PropValue::Str(value.to_owned()))
            }
            op::SET_PROP_NUM => {
                let id = reader.u32()?;
                let key = reader.str()?;
                let value = reader.f64()?;
                tree.set_prop(id, key, PropValue::Number(value))
            }
            op::SET_PROP_BOOL => {
                let id = reader.u32()?;
                let key = reader.str()?;
                let value = reader.u8()? != 0;
                tree.set_prop(id, key, PropValue::Bool(value))
            }
            op::SET_PROP_NULL => {
                let id = reader.u32()?;
                let key = reader.str()?;
                tree.set_prop(id, key, PropValue::Null)
            }
            op::SET_TEXT => {
                let id = reader.u32()?;
                let text = reader.str()?;
                tree.set_text(id, text)
            }
            op::SET_LISTENER => {
                let id = reader.u32()?;
                let event = reader.str()?;
                let enabled = reader.u8()? != 0;
                tree.set_listener(id, event, enabled)
            }
            op::SET_ROOT => tree.set_root(reader.u32()?),
            other => return Err(ProtocolError::UnknownOpcode { opcode: other, offset: start }),
        };
        result.map_err(|error| ProtocolError::Tree { opcode, offset: start, error })?;
        applied += 1;
    }
    Ok(applied)
}

/// An encoder for the same format, in Rust. The runtime does not use it — it
/// exists so the decoder can be tested without starting a JS engine, and so any
/// change to the format breaks the tests at both ends at once.
#[derive(Default)]
pub struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn str(&mut self, value: &str) {
        self.bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn create_node(&mut self, id: u32, kind: NodeKind) -> &mut Self {
        self.bytes.push(op::CREATE_NODE);
        self.u32(id);
        self.bytes.push(kind_to_byte(kind));
        self
    }

    pub fn destroy_node(&mut self, id: u32) -> &mut Self {
        self.bytes.push(op::DESTROY_NODE);
        self.u32(id);
        self
    }

    pub fn insert_child(&mut self, parent: u32, child: u32, index: u32) -> &mut Self {
        self.bytes.push(op::INSERT_CHILD);
        self.u32(parent);
        self.u32(child);
        self.u32(index);
        self
    }

    pub fn remove_child(&mut self, parent: u32, child: u32) -> &mut Self {
        self.bytes.push(op::REMOVE_CHILD);
        self.u32(parent);
        self.u32(child);
        self
    }

    pub fn set_style(&mut self, id: u32, name: &str, value: &str) -> &mut Self {
        self.bytes.push(op::SET_STYLE);
        self.u32(id);
        self.str(name);
        self.str(value);
        self
    }

    pub fn set_prop_str(&mut self, id: u32, key: &str, value: &str) -> &mut Self {
        self.bytes.push(op::SET_PROP_STR);
        self.u32(id);
        self.str(key);
        self.str(value);
        self
    }

    pub fn set_prop_num(&mut self, id: u32, key: &str, value: f64) -> &mut Self {
        self.bytes.push(op::SET_PROP_NUM);
        self.u32(id);
        self.str(key);
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn set_prop_bool(&mut self, id: u32, key: &str, value: bool) -> &mut Self {
        self.bytes.push(op::SET_PROP_BOOL);
        self.u32(id);
        self.str(key);
        self.bytes.push(value as u8);
        self
    }

    /// `null` means "go back to your factory value", not "send nothing": the
    /// host has to find out that something it did write has been taken away.
    pub fn set_prop_null(&mut self, id: u32, key: &str) -> &mut Self {
        self.bytes.push(op::SET_PROP_NULL);
        self.u32(id);
        self.str(key);
        self
    }

    pub fn set_text(&mut self, id: u32, text: &str) -> &mut Self {
        self.bytes.push(op::SET_TEXT);
        self.u32(id);
        self.str(text);
        self
    }

    pub fn set_listener(&mut self, id: u32, event: &str, enabled: bool) -> &mut Self {
        self.bytes.push(op::SET_LISTENER);
        self.u32(id);
        self.str(event);
        self.bytes.push(enabled as u8);
        self
    }

    pub fn set_root(&mut self, id: u32) -> &mut Self {
        self.bytes.push(op::SET_ROOT);
        self.u32(id);
        self
    }
}
