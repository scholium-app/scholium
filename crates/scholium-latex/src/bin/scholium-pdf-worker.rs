//! Fixed PDF decoder entry point; invoked only through scholium-build's decoder policy.
fn main() -> Result<(), scholium_latex::LatexError> {
    scholium_latex::view::pdf_worker_entry()
}
