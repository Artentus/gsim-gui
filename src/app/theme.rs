use egui::ImageSource;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

pub struct ThemedImage {
    light: ImageSource<'static>,
    dark: ImageSource<'static>,
}

impl ThemedImage {
    #[inline]
    pub const fn new(light: ImageSource<'static>, dark: ImageSource<'static>) -> Self {
        Self { light, dark }
    }

    #[inline]
    pub fn source(&self, theme: Theme) -> ImageSource<'static> {
        match theme {
            Theme::Light => self.light.clone(),
            Theme::Dark => self.dark.clone(),
        }
    }
}

macro_rules! themed_image {
    ($name:ident.png) => {{
        const LIGHT: egui::ImageSource<'static> = egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://", stringify!($name), "_Light.png")),
            bytes: egui::load::Bytes::Static(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/images/light/",
                stringify!($name),
                ".png",
            ))),
        };

        const DARK: egui::ImageSource<'static> = egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://", stringify!($name), "_Dark.png")),
            bytes: egui::load::Bytes::Static(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/images/dark/",
                stringify!($name),
                ".png",
            ))),
        };

        static IMAGE: ThemedImage = ThemedImage::new(LIGHT, DARK);
        &IMAGE
    }};
    ($name:ident.svg) => {{
        const LIGHT: egui::ImageSource<'static> = egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://", stringify!($name), "_Light.svg")),
            bytes: egui::load::Bytes::Static(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/images/light/",
                stringify!($name),
                ".svg",
            ))),
        };

        const DARK: egui::ImageSource<'static> = egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://", stringify!($name), "_Dark.svg")),
            bytes: egui::load::Bytes::Static(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/images/dark/",
                stringify!($name),
                ".svg",
            ))),
        };

        static IMAGE: ThemedImage = ThemedImage::new(LIGHT, DARK);
        &IMAGE
    }};
}
