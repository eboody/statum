use super::{TransitionDescriptor, TransitionPresentation};

/// Runtime accessor for transition descriptors that may be supplied by a
/// distributed registration surface.
#[derive(Clone, Copy)]
pub struct TransitionInventory<S: 'static, T: 'static> {
    get: fn() -> &'static [TransitionDescriptor<S, T>],
}

impl<S, T> TransitionInventory<S, T> {
    /// Creates a transition inventory from a `'static` getter.
    pub const fn new(get: fn() -> &'static [TransitionDescriptor<S, T>]) -> Self {
        Self { get }
    }

    /// Returns the transition descriptors as a slice.
    pub fn as_slice(&self) -> &'static [TransitionDescriptor<S, T>] {
        (self.get)()
    }
}

impl<S, T> core::ops::Deref for TransitionInventory<S, T> {
    type Target = [TransitionDescriptor<S, T>];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<S, T> core::fmt::Debug for TransitionInventory<S, T> {
    fn fmt(
        &self,
        formatter: &mut core::fmt::Formatter<'_>,
    ) -> core::result::Result<(), core::fmt::Error> {
        formatter.debug_tuple("TransitionInventory").finish()
    }
}

impl<S, T> core::cmp::PartialEq for TransitionInventory<S, T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.as_slice(), other.as_slice())
    }
}

impl<S, T> core::cmp::Eq for TransitionInventory<S, T> {}

/// Runtime accessor for transition presentation metadata that may be supplied
/// by a distributed registration surface.
#[derive(Clone, Copy)]
pub struct TransitionPresentationInventory<T: 'static, M: 'static = ()> {
    get: fn() -> &'static [TransitionPresentation<T, M>],
}

impl<T, M> TransitionPresentationInventory<T, M> {
    /// Creates a transition presentation inventory from a `'static` getter.
    pub const fn new(get: fn() -> &'static [TransitionPresentation<T, M>]) -> Self {
        Self { get }
    }

    /// Returns the transition presentation descriptors as a slice.
    pub fn as_slice(&self) -> &'static [TransitionPresentation<T, M>] {
        (self.get)()
    }
}

impl<T, M> core::ops::Deref for TransitionPresentationInventory<T, M> {
    type Target = [TransitionPresentation<T, M>];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, M> core::fmt::Debug for TransitionPresentationInventory<T, M> {
    fn fmt(
        &self,
        formatter: &mut core::fmt::Formatter<'_>,
    ) -> core::result::Result<(), core::fmt::Error> {
        formatter
            .debug_tuple("TransitionPresentationInventory")
            .finish()
    }
}

impl<T, M> core::cmp::PartialEq for TransitionPresentationInventory<T, M> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.as_slice(), other.as_slice())
    }
}

impl<T, M> core::cmp::Eq for TransitionPresentationInventory<T, M> {}
