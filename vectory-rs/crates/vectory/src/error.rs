use std::fmt;

/// Decode/serialize errors for the SVG model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The input is not well-formed XML (leptris rejects it too).
    Xml(String),
    /// The input parsed but has no root element (mirrors vectory's
    /// `ParsingError` on `document.root` being nil).
    NoRoot,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Xml(detail) => write!(f, "invalid XML: {detail}"),
            Error::NoRoot => write!(f, "invalid XML: no root element"),
        }
    }
}

impl std::error::Error for Error {}

impl From<quick_xml::errors::Error> for Error {
    fn from(value: quick_xml::errors::Error) -> Self {
        Error::Xml(value.to_string())
    }
}

impl From<quick_xml::escape::EscapeError> for Error {
    fn from(value: quick_xml::escape::EscapeError) -> Self {
        Error::Xml(format!("entity decoding: {value}"))
    }
}

impl From<quick_xml::events::attributes::AttrError> for Error {
    fn from(value: quick_xml::events::attributes::AttrError) -> Self {
        Error::Xml(format!("attribute: {value}"))
    }
}
