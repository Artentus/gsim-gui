use egui::{Context, TextureId, Vec2};
use egui_extras::RetainedImage;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

pub struct ThemedImage {
    light: RetainedImage,
    dark: RetainedImage,
}

impl ThemedImage {
    #[inline]
    pub fn new(light: RetainedImage, dark: RetainedImage) -> Self {
        Self { light, dark }
    }

    pub fn texture_id(&self, ctx: &Context, theme: Theme) -> TextureId {
        match theme {
            Theme::Light => self.light.texture_id(ctx),
            Theme::Dark => self.dark.texture_id(ctx),
        }
    }

    #[inline]
    pub fn size_vec2(&self) -> Vec2 {
        self.light.size_vec2()
    }
}

macro_rules! themed_image {
    ($name:ident.png) => {{
        use egui_extras::RetainedImage;
        use tracing_unwrap::ResultExt;

        const LIGHT_NAME: &str = concat!(stringify!($name), "_Light");
        const LIGHT_DATA: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/images/light/",
            stringify!($name),
            ".png"
        ));

        const DARK_NAME: &str = concat!(stringify!($name), "_Dark");
        const DARK_DATA: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/images/dark/",
            stringify!($name),
            ".png"
        ));

        let light = RetainedImage::from_image_bytes(LIGHT_NAME, LIGHT_DATA)
            .expect_or_log(concat!(stringify!($name), ".png: invalid image data"));

        let dark = RetainedImage::from_image_bytes(DARK_NAME, DARK_DATA)
            .expect_or_log(concat!(stringify!($name), ".png: invalid image data"));

        assert_eq!(light.size(), dark.size());
        ThemedImage::new(light, dark)
    }};
    ($name:ident.svg) => {{
        use egui_extras::RetainedImage;
        use tracing_unwrap::ResultExt;

        const LIGHT_NAME: &str = concat!(stringify!($name), "_Light");
        const LIGHT_DATA: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/images/light/",
            stringify!($name),
            ".svg"
        ));

        const DARK_NAME: &str = concat!(stringify!($name), "_Dark");
        const DARK_DATA: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/images/dark/",
            stringify!($name),
            ".svg"
        ));

        let light = RetainedImage::from_svg_bytes(LIGHT_NAME, LIGHT_DATA)
            .expect_or_log(concat!(stringify!($name), ".svg: invalid image data"));

        let dark = RetainedImage::from_svg_bytes(DARK_NAME, DARK_DATA)
            .expect_or_log(concat!(stringify!($name), ".svg: invalid image data"));

        assert_eq!(light.size(), dark.size());
        ThemedImage::new(light, dark)
    }};
}
