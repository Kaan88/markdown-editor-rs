//! The plan screen (`docs/ai-library-plan.md` 5.3). The card, its state, its events and its
//! drawing live in the shared crates (`mdshared_ui::plan`, D-081) and are kept here by their old
//! names, so `crate::plan_ui` reads as before. What is mdedit's is where the headings come from:
//! the function list, which the shared card does not depend on. The window's wiring is
//! `app/plan.rs`, the writer `app/plan_write.rs`.
//!
//! Nothing here exists until a plan is opened, so it is nothing on the launch path.

use mdedit_core::buffer::Buffer;
use mdedit_core::split::Heading;

pub use mdshared_ui::plan::*;

/// The most headings read from a document.
const HEADINGS_MAX: usize = 200_000;

/// The headings of a document as the function list finds them (it skips fences and front
/// matter): a line and a level, for `split::plan`.
pub fn headings_of(doc: &Buffer) -> Vec<Heading> {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let (symbols, _) = mdedit_outline::symbols(
        doc.rope(),
        mdedit_outline::Flavor::Markdown,
        HEADINGS_MAX,
        &cancel,
    );
    symbols
        .iter()
        .filter(|s| s.kind == mdedit_outline::Kind::Heading)
        .map(|s| Heading {
            line: s.line,
            level: s.depth.saturating_add(1),
        })
        .collect()
}
