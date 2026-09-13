#[path = "impls/i18n.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the i18n implementation path stable"
)]
mod i18n;

pub(crate) use i18n::*;
