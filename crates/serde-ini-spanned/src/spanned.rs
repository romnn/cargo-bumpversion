pub type FileId = usize;
pub type Span = std::ops::Range<usize>;

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub inner: T,
    pub span: Span,
}

impl std::borrow::Borrow<String> for Spanned<String> {
    fn borrow(&self) -> &String {
        &self.inner
    }
}

impl std::borrow::Borrow<str> for Spanned<String> {
    fn borrow(&self) -> &str {
        self.inner.as_str()
    }
}

impl<T> std::ops::Deref for Spanned<T>
where
    T: std::ops::Deref,
{
    type Target = <T as std::ops::Deref>::Target;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

pub trait DerefInner {
    type Target: ?Sized;
    fn deref_inner(&self) -> Option<&Self::Target>;
}

impl<T> DerefInner for Option<&T>
where
    T: std::ops::Deref,
{
    type Target = <T as std::ops::Deref>::Target;

    fn deref_inner(&self) -> Option<&Self::Target> {
        self.map(std::ops::Deref::deref)
    }
}

impl<T> AsRef<T> for Spanned<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> Spanned<T> {
    pub fn new(span: impl Into<Span>, value: T) -> Self {
        Self {
            span: span.into(),
            inner: value,
        }
    }

    pub const fn dummy(value: T) -> Self {
        Self {
            span: Span { start: 0, end: 0 },
            inner: value,
        }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> std::fmt::Display for Spanned<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}

impl<T> PartialEq for Spanned<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> PartialEq<T> for Spanned<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &T) -> bool {
        (&self.inner as &dyn PartialEq<T>).eq(other)
    }
}

impl<T> PartialEq<&T> for Spanned<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &&T) -> bool {
        (&self.inner as &dyn PartialEq<T>).eq(*other)
    }
}

impl PartialEq<str> for Spanned<String> {
    fn eq(&self, other: &str) -> bool {
        std::cmp::PartialEq::eq(self.as_ref().as_str(), other)
    }
}

impl PartialEq<str> for &Spanned<String> {
    fn eq(&self, other: &str) -> bool {
        std::cmp::PartialEq::eq(self.as_ref().as_str(), other)
    }
}

impl<T> Ord for Spanned<T>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        Ord::cmp(&self.inner, &other.inner)
    }
}

impl<T> PartialOrd for Spanned<T>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        PartialOrd::partial_cmp(&self.inner, &other.inner)
    }
}

impl<T> PartialOrd<T> for Spanned<T>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        PartialOrd::partial_cmp(&self.inner, other)
    }
}

impl<T> Eq for Spanned<T> where T: Eq {}

impl<T> std::hash::Hash for Spanned<T>
where
    T: std::hash::Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}
