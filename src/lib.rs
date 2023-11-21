mod app;
pub use app::App;

macro_rules! size_of {
    ($t:ty) => {
        std::mem::size_of::<$t>()
    };
}
pub(crate) use size_of;

macro_rules! is_discriminant {
    ($value:expr, $discriminant:path) => {
        match &$value {
            $discriminant { .. } => true,
            _ => false,
        }
    };
}
pub(crate) use is_discriminant;

#[allow(dead_code)]
pub(crate) type HashMap<K, V> = ahash::AHashMap<K, V>;

#[allow(dead_code)]
pub(crate) type HashSet<T> = ahash::AHashSet<T>;
