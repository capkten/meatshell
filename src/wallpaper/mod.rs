#[path = "impls/wallpaper.rs"]
#[expect(
    clippy::module_inception,
    reason = "keep the wallpaper implementation path stable"
)]
mod wallpaper;
#[path = "struct/wallpaper.rs"]
mod wallpaper_types;

pub(crate) use wallpaper::*;
