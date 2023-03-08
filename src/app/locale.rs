use fluent::{FluentBundle, FluentResource};
use std::borrow::Cow;
use std::collections::HashMap;

pub use unic_langid::langid;
pub use unic_langid::LanguageIdentifier as LangId;

#[repr(transparent)]
struct Locale {
    bundle: FluentBundle<FluentResource>,
}

impl Locale {
    fn load(lang: LangId, source: String) -> Self {
        let res = match FluentResource::try_new(source) {
            Ok(res) => res,
            Err((res, errors)) => {
                for err in errors {
                    tracing::error!(%err);
                }

                res
            }
        };

        let mut bundle = FluentBundle::new(vec![lang]);
        bundle.add_resource(res).expect("failed to add resource");

        Self { bundle }
    }

    fn get<'a>(&'a self, key: &'static str) -> Option<Cow<'a, str>> {
        let msg = self.bundle.get_message(key)?;
        let pattern = msg.value()?;
        let mut errors = vec![];
        let value = self.bundle.format_pattern(pattern, None, &mut errors);

        if errors.len() > 0 {
            let mut error_value = String::new();

            for err in errors {
                let fluent::FluentError::ResolverError(err) = err else {
                    panic!("unexpected error kind");
                };

                if error_value.len() > 0 {
                    error_value.push('\n');
                }
                error_value.push_str(&format!("{err}"));
            }

            return Some(error_value.into());
        }

        Some(value)
    }
}

pub const DEFAULT_LANG: LangId = langid!("en");

macro_rules! locale {
    ($locales:expr, $lang:literal) => {{
        const SOURCE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/lang/",
            $lang,
            ".ftl"
        ));

        let lang = langid!($lang);
        let locale = Locale::load(lang.clone(), SOURCE.to_owned());
        $locales.insert(lang, locale);
    }};
}

#[repr(transparent)]
pub struct LocaleManager {
    locales: HashMap<LangId, Locale>,
}

impl LocaleManager {
    pub fn init() -> Self {
        let mut locales = HashMap::new();

        locale!(locales, "en");
        locale!(locales, "de");

        assert!(locales.get(&DEFAULT_LANG).is_some());
        Self { locales }
    }

    #[inline]
    pub fn langs(&self) -> impl Iterator<Item = &LangId> {
        let mut langs: Vec<_> = self.locales.keys().collect();
        langs.sort_by_cached_key(|&lang| self.locales[lang].get("english-lang-name"));
        langs.into_iter()
    }

    fn get_default<'a>(&'a self, key: &'static str) -> Cow<'a, str> {
        let locale = &self.locales[&DEFAULT_LANG];
        locale.get(key).unwrap_or(key.into())
    }

    pub fn get<'a>(&'a self, lang: &LangId, key: &'static str) -> Cow<'a, str> {
        self.locales
            .get(&lang)
            .and_then(|locale| locale.get(key))
            .unwrap_or_else(|| self.get_default(key))
    }
}
