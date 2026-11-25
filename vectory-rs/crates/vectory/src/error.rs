use std::fmt;

/// Decode/serialize errors for the SVG model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The input is not well-formed XML (leptris rejects it too).
    Xml(String),
    /// The input parsed but has no root element (mirrors vectory's
    /// `ParsingError` on `document.root` being nil).
    NoRoot,
    /// Ruby `Vectory::ParsingError`: a serialized boundary failed to parse
    /// (graft envelopes, pipeline inputs).
    Parsing(String),
    /// Ruby `ArgumentError`: an invalid transform sequence, scope mismatch,
    /// or invalid namespace request.
    Transform(String),
    /// Ruby `ConversionError`: unparseable data URIs and unknown mime types.
    Conversion(String),
    /// Ruby `ForbiddenExternalReferenceError`: the input policy refused the
    /// content before parsing (DTD internal subsets).
    ForbiddenExternalReference(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Xml(detail) => write!(f, "invalid XML: {detail}"),
            Error::NoRoot => write!(f, "invalid XML: no root element"),
            Error::Parsing(detail) => write!(f, "parsing: {detail}"),
            Error::Transform(detail) => write!(f, "transform: {detail}"),
            Error::Conversion(detail) => write!(f, "conversion: {detail}"),
            Error::ForbiddenExternalReference(detail) => {
                write!(f, "forbidden external reference: {detail}")
            }
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
