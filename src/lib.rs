mod app;
pub use app::App;

macro_rules! size_of {
    ($t:ty) => {
        std::mem::size_of::<$t>()
    };
}
pub(crate) use size_of;

#[allow(dead_code)]
pub(crate) type HashMap<K, V> = ahash::AHashMap<K, V>;

#[allow(dead_code)]
pub(crate) type HashSet<T> = ahash::AHashSet<T>;
