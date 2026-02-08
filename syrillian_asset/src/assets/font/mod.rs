use crate::assets::HFont;
use crate::store::{H, HandleName, Store, StoreDefaults, StoreType, StoreTypeFallback};
use crate::store_add_checked;
use std::convert::Into;
use std::sync::Arc;
use tracing::trace;

#[derive(Debug, Clone)]
pub struct Font {
    pub family_name: String,
    pub font_bytes: Arc<Vec<u8>>,
    pub atlas_em_px: u32,
}

impl StoreType for Font {
    fn name() -> &'static str {
        "Font"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_: H<Self>) -> bool {
        false
    }
}

impl H<Font> {
    const DEFAULT_ID: u32 = 0;
    pub const DEFAULT: HFont = HFont::new(Self::DEFAULT_ID);
}

impl StoreDefaults for Font {
    fn populate(store: &mut Store<Self>) {
        store_add_checked!(
            store,
            HFont::DEFAULT_ID,
            Font::new("Noto Sans", None).expect("Default font should always be loadable")
        );
    }
}

impl StoreTypeFallback for Font {
    fn fallback() -> H<Self> {
        HFont::DEFAULT
    }
}

impl<T: StoreTypeFallback> Default for H<T> {
    fn default() -> Self {
        T::fallback()
    }
}

pub const DEFAULT_ATLAS_SIZE: u32 = 1024;

impl Font {
    /// The default atlas glyph size is 1024 pixels
    pub fn new(family_name: impl Into<String>, atlas_em_px: Option<u32>) -> Option<Self> {
        let family_name = family_name.into();
        let atlas_em_px = atlas_em_px.unwrap_or(DEFAULT_ATLAS_SIZE);
        let bytes = find_font_and_bytes(&family_name)?;
        Some(Self {
            family_name,
            font_bytes: bytes,
            atlas_em_px,
        })
    }
}

impl Store<Font> {
    #[inline]
    pub fn load(&self, font_family: &str, atlas_em_px: Option<u32>) -> Option<HFont> {
        if let Some(font) = self.find(font_family) {
            return Some(font);
        }

        let loaded_font = Font::new(font_family, atlas_em_px)?;
        Some(self.add(loaded_font))
    }

    pub fn find(&self, family_name: &str) -> Option<HFont> {
        self.items()
            .find(|item| item.family_name == family_name)
            .map(|item| (*item.key()).into())
    }
}

static NOTO_SANS_REGULAR: &[u8] = include_bytes!("NotoSans-Regular.ttf");

#[cfg(target_arch = "wasm32")]
fn find_font_and_bytes(_family_name: &str) -> Option<Arc<Vec<u8>>> {
    Some(Arc::new(NOTO_SANS_REGULAR.to_vec()))
}

#[cfg(not(target_arch = "wasm32"))]
fn find_font_and_bytes(family_name: &str) -> Option<Arc<Vec<u8>>> {
    use once_cell::sync::OnceCell;

    static DB: OnceCell<fontdb::Database> = OnceCell::new();
    let db = DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db.load_fonts_dir(".");
        db.load_font_data(NOTO_SANS_REGULAR.to_vec());
        for face in db.faces() {
            trace!("Loaded font: {}", face.post_script_name);
        }
        db
    });

    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family_name)],
        ..Default::default()
    };

    let face_id = db.query(&query)?;

    let bytes = match db.face_source(face_id)?.0 {
        fontdb::Source::Binary(b) => (*b).as_ref().to_vec(),
        fontdb::Source::File(path) => std::fs::read(path).ok()?,
        fontdb::Source::SharedFile(path, ..) => std::fs::read(path).ok()?,
    };

    Some(Arc::new(bytes))
}
