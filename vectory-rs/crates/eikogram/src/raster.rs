//! Raster references: embedded images as opaque payload handles. Pixel data
//! itself belongs to the raster side (claricle/emf#89); the VIM carries only
//! the resolved payload plus its wire format.
#![allow(dead_code)]

/// The payload format of a raster reference. JPEG carries whether its payload
/// was zero-padded to a triple boundary (a reference-emission quirk that
/// changes the encoded form; the producing transformer resolves it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RasterFormat {
    Png,
    Jpeg { zero_padded: bool },
}

/// An embedded raster payload: opaque bytes plus their format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub format: RasterFormat,
    pub payload: Vec<u8>,
}
