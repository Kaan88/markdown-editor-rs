//! Compare two files.

use crate::commands::CommandId as C;
use crate::docs::Docs;

pub fn enabled(_id: C, _docs: &Docs) -> bool {
    true
}
