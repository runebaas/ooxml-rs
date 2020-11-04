//! There's some marker trait and helper trait for XML (de)serializing.
//!
use std::io::BufReader;
use std::marker::Sized;
use std::{borrow::Cow, fs::File, io::prelude::*, path::Path};

use crate::error::OoxmlError;

use quick_xml::events::attributes::Attribute;
use quick_xml::events::*;

use super::namespace::Namespaces;

/// Leaf for plain text, Node for internal xml element, Root for root element of a Part.
pub enum OpenXmlElementType {
    Leaf,
    Node,
    Root,
}

pub trait OpenXmlLeafElement {}
pub trait OpenXmlNodeElement {}
pub trait OpenXmlRootElement {}

/// Static information of an OpenXml element
pub trait OpenXmlElementInfo: Sized {
    /// XML tag name
    fn tag_name() -> &'static str;

    fn as_bytes_start() -> quick_xml::events::BytesStart<'static> {
        assert!(Self::have_tag_name());
        quick_xml::events::BytesStart::borrowed_name(Self::tag_name().as_bytes())
    }
    fn as_bytes_end() -> quick_xml::events::BytesEnd<'static> {
        assert!(Self::have_tag_name());
        quick_xml::events::BytesEnd::borrowed(Self::tag_name().as_bytes())
    }

    fn is_leaf_text_element() -> bool {
        match Self::element_type() {
            OpenXmlElementType::Leaf => true,
            _ => false,
        }
    }
    fn is_root_element() -> bool {
        match Self::element_type() {
            OpenXmlElementType::Root => true,
            _ => false,
        }
    }

    /// Element type
    fn element_type() -> OpenXmlElementType {
        OpenXmlElementType::Node
    }

    /// If the element have a tag name.
    ///
    /// Specially, plain text or cdata element does not have tag name.
    fn have_tag_name() -> bool {
        match Self::element_type() {
            OpenXmlElementType::Leaf => false,
            _ => true,
        }
    }

    /// If the element can have namespace declartions.
    fn can_have_namespace_declarations() -> bool {
        match Self::element_type() {
            OpenXmlElementType::Leaf => false,
            _ => true,
        }
    }
    /// If the element can have attributes.
    fn can_have_attributes() -> bool {
        match Self::element_type() {
            OpenXmlElementType::Leaf => false,
            _ => true,
        }
    }
    /// If the element can have children.
    ///
    /// Eg. plain text element cannot have children, so all children changes not works.
    fn can_have_children() -> bool {
        match Self::element_type() {
            OpenXmlElementType::Leaf => false,
            _ => true,
        }
    }
}

/// Common element trait.
pub trait OpenXmlElementExt: OpenXmlElementInfo {
    /// Get element attributes, if have.
    fn attributes(&self) -> Option<Vec<Attribute>>;

    /// Get element namespaces, if have.
    fn namespaces(&self) -> Option<Cow<Namespaces>>;

    /// Serialize to writer
    fn write_inner<W: Write>(&self, writer: W) -> crate::error::Result<()>;

    fn write_outter<W: Write>(&self, writer: W) -> crate::error::Result<()> {
        let mut writer = quick_xml::Writer::new(writer);
        use quick_xml::events::*;

        if Self::is_root_element() {
            writer.write_event(Event::Decl(BytesDecl::new(
                b"1.0",
                Some(b"UTF-8"),
                Some(b"yes"),
            )))?;
        }
        let mut elem = Self::as_bytes_start();
        if Self::can_have_namespace_declarations() {
            if let Some(ns) = self.namespaces() {
                elem.extend_attributes(ns.to_xml_attributes());
            }
        }
        if Self::can_have_attributes() {
            if let Some(attrs) = self.attributes() {
                elem.extend_attributes(attrs);
            }
        }
        if Self::is_leaf_text_element() {
            writer.write_event(Event::Empty(elem))?;
            return Ok(());
        } else {
            writer.write_event(Event::Start(elem))?;
            self.write_inner(writer.inner())?;
            writer.write_event(Event::End(Self::as_bytes_end()))?;
            return Ok(());
        }
    }
}

pub trait FromXml: Sized {
    /// Parse from an xml stream reader
    fn from_xml_reader<R: BufRead>(reader: R) -> Result<Self, OoxmlError>;

    /// Parse from an xml raw string.
    fn from_xml_str(s: &str) -> Result<Self, OoxmlError> {
        Self::from_xml_reader(s.as_bytes())
    }

    /// Open a OpenXML file path, parse everything into the memory.
    fn from_xml_file<P: AsRef<Path>>(path: P) -> Result<Self, OoxmlError> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        Self::from_xml_reader(reader)
    }
}

pub trait OpenXmlFromDeserialize: serde::de::DeserializeOwned {}

impl<T: OpenXmlFromDeserialize> FromXml for T {
    fn from_xml_reader<R: BufRead>(reader: R) -> Result<Self, OoxmlError> {
        Ok(quick_xml::de::from_reader(reader)?)
    }
}
pub trait ToXml: Sized {
    /// Write inner xml
    fn writer_inner<W: Write>(&self, writer: W) -> Result<(), OoxmlError> {
        unimplemented!()
    }

    /// Writer outer xml
    // fn write_outter<W: Write>(&self, writer: W) -> Result<(), OoxmlError> {
    // unimplemented!()
    // }

    /// Implement the write method, the trait will do the rest.
    fn write<W: Write>(&self, writer: W) -> Result<(), OoxmlError>;

    //fn write_inner_xml(&self)

    /// Write standalone xml to the writer.
    ///
    /// Will add `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` to writer.
    fn write_standalone<W: Write>(&self, writer: W) -> Result<(), OoxmlError> {
        let mut xml = quick_xml::Writer::new(writer);
        use quick_xml::events::*;
        xml.write_event(Event::Decl(BytesDecl::new(
            b"1.0",
            Some(b"UTF-8"),
            Some(b"yes"),
        )))?;
        self.write(xml.inner())
    }
    /// Write the standalone xml to path
    fn save_as<P: AsRef<Path>>(&self, path: P) -> Result<(), OoxmlError> {
        let file = File::open(path)?;
        self.write_standalone(file)
    }
    /// Output the xml to an Vec<u8> block.
    fn to_xml_bytes(&self) -> Result<Vec<u8>, OoxmlError> {
        let mut container = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut container);
        self.write(&mut cursor)?;
        Ok(container)
    }
    /// Output the xml to string.
    fn to_xml_string(&self) -> Result<String, OoxmlError> {
        let bytes = self.to_xml_bytes()?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

pub trait OpenXmlSerializeTo: serde::ser::Serialize + OpenXmlElementInfo {}

// impl<T: OpenXmlSerializeTo> ToXml for T {
//     fn write<W: Write>(&self, writer: W) -> Result<(), OoxmlError> {
//         let mut writer = quick_xml::Writer::new(writer);
//         let mut ser = quick_xml::se::Serializer::with_root(writer, Some(Self::tag_name()));
//         self.serialize(&mut ser)?;
//         //quick_xml::se::to_writer(writer, self)?;
//         Ok(())
//     }
// }

impl<T: OpenXmlElementExt> ToXml for T {
    fn write<W: Write>(&self, writer: W) -> Result<(), OoxmlError> {
        self.write_outter(writer)
    }
}

pub trait OpenXmlElement: FromXml + ToXml + OpenXmlElementInfo {
    fn tag(&self) -> &[u8];
    fn namespace_declarations(&self) -> Vec<Attribute>;
    fn add_namespace_declaration(&mut self, prefix: &str, uri: &str);
    fn remove_namespace_declaration(&mut self, prefix: &str);

    //fn markup_compatibility_attributes(&self) -> ();
    fn extended_attributes(&self) -> Vec<Attribute>;
    fn has_attributes(&self) -> bool;
    fn set_attribute(&mut self, attribute: Attribute);
    fn remove_attribute(&mut self, local_name: &str, namespace_uri: &str);
    fn clear_attributes(&mut self);

    fn has_children(&self) -> bool;
    //type Children;

    //fn append(&mut self, children: OpenXmlChild);
    //fn clear_children(&mut self);
    fn write_children<W: Write>(&self, writer: W) -> Result<(), OoxmlError>;

    // FIXME(@zitsen): need a OpenXmlAttribute definition.
    fn get_attribute(&self, name: &str);
    // FIXME(@zitsen): there's other implmentations for children elements.
    fn children<'xml, X>(&self) -> Box<dyn Iterator<Item = &'xml X>>
    where
        X: OpenXmlElement,
    {
        unimplemented!()
    }

    fn write<W: Write>(&self, mut writer: W) -> Result<(), OoxmlError> {
        let mut xml = quick_xml::Writer::new(&mut writer);
        // 2. start types element
        let tag = Self::tag_name();
        let mut elem = BytesStart::borrowed_name(tag.as_bytes());
        let attrs = self.extended_attributes();
        if !attrs.is_empty() {
            elem.extend_attributes(attrs);
        }

        xml.write_event(Event::Start(elem))?;

        if self.has_children() {
            self.write_children(xml.inner())?;
        }

        // ends types element.
        let end = BytesEnd::borrowed(tag.as_bytes());
        xml.write_event(Event::End(end))?;
        Ok(())
    }
}

/// Expose trait for an element implemented serde deserialize trait, make it simple and fast.
pub trait OpenXmlElementDeserialize: OpenXmlElement + serde::de::DeserializeOwned {
    fn from_xml_reader<R: BufRead>(reader: R) -> Result<Self, OoxmlError> {
        Ok(quick_xml::de::from_reader(reader)?)
    }
    fn from_xml_str(s: &str) -> Result<Self, OoxmlError> {
        Ok(quick_xml::de::from_str(s)?)
    }
}
