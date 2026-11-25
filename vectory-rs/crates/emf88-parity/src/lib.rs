//! # emf88-parity — the byte-parity acceptance harness
//!
//! Binds the two transformers (EMF-model → VIM, VIM → SVG-model) to run
//! THE GATE from claricle/emf#88: the full transformer chain must equal
//! the Ruby renderer's SVG bytes FILE-BY-FILE over the emfsvg parity
//! corpus. This crate is the acceptance harness only — no production code;
//! the transformers themselves carry no reference to each other.

#[cfg(test)]
mod gate {
    #[test]
    fn wired() {
        // The harness compiles against both transformers; the corpus gate
        // lives in tests/parity_corpus.rs.
    }
}
