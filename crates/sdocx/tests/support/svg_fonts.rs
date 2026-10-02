use resvg::usvg;
use sdocx::fonts::fontdb;

#[cfg(feature = "pdf")]
pub fn font_resolver(database: &fontdb::Database) -> usvg::FontResolver<'static> {
    sdocx::fonts::svg_font_resolver(database)
}

#[cfg(not(feature = "pdf"))]
pub fn font_resolver(database: &fontdb::Database) -> usvg::FontResolver<'static> {
    let families = sdocx::fonts::SvgFontFamilies::new(database);
    let default_selector = usvg::FontResolver::default_font_selector();
    usvg::FontResolver {
        select_font: Box::new(move |font, database| {
            if let Some(usvg::FontFamily::Named(family)) = font.families().first()
                && let Some(id) = families.resolve(family)
            {
                return Some(id);
            }
            default_selector(font, database)
        }),
        ..Default::default()
    }
}
