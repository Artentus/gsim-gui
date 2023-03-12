mod app;
pub use app::App;

macro_rules! size_of {
    ($t:ty) => {
        std::mem::size_of::<$t>()
    };
}
pub(crate) use size_of;
