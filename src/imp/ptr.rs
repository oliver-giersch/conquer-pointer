//! Implementation for [`MarkedPtr`].

use core::cmp;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::ptr::{self, NonNull};

use crate::{MarkedNonNull, MarkedPtr};

/********** impl Clone ****************************************************************************/

impl<T, const N: usize> Clone for MarkedPtr<T, N> {
    #[inline]
    fn clone(&self) -> Self {
        Self { inner: self.inner }
    }
}

/********** impl Copy *****************************************************************************/

impl<T, const N: usize> Copy for MarkedPtr<T, N> {}

/********** impl inherent *************************************************************************/

impl<T, const N: usize> MarkedPtr<T, N> {
    /// The number of available mark bits for this type.
    pub const TAG_BITS: usize = N;
    /// The bitmask for the lower markable bits.
    pub const TAG_MASK: usize = crate::mark_mask::<T>(Self::TAG_BITS);
    /// The bitmask for the (higher) pointer bits.
    pub const POINTER_MASK: usize = !Self::TAG_MASK;

    /// Creates a new unmarked `null` pointer.
    #[inline]
    pub const fn null() -> Self {
        Self::new(ptr::null_mut())
    }

    /// Creates a new unmarked [`MarkedPtr`].
    #[inline]
    pub const fn new(ptr: *mut T) -> Self {
        Self { inner: ptr }
    }

    /// Creates a [`MarkedPtr`] from the integer (numeric) representation of a
    /// potentially marked pointer.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::ptr;
    ///
    /// type MarkedPtr = conquer_pointer::MarkedPtr<i32, 1>;
    ///
    /// let ptr = MarkedPtr::from_usize(1);
    /// assert_eq!(ptr.decompose(), (ptr::null_mut(), 1));
    /// ```
    #[inline]
    pub const fn from_usize(val: usize) -> Self {
        Self { inner: val as *mut _ }
    }

    /// Composes a new [`MarkedPtr`] from a raw `ptr` and a `tag` value.
    ///
    /// The supplied `ptr` is assumed to be well-aligned (i.e. has no tag bits
    /// set), so this function may lead to unexpected results when this is not
    /// the case. 
    /// 
    /// # Examples
    ///
    /// ```
    /// use core::ptr;
    ///
    /// type MarkedPtr = conquer_pointer::MarkedPtr<i32, 2>;
    ///
    /// let raw = &1 as *const i32 as *mut i32;
    /// let ptr = MarkedPtr::compose(raw, 0b11);
    /// assert_eq!(ptr.decompose(), (raw, 0b11));
    /// // any excess bits are silently truncated.
    /// let ptr = MarkedPtr::compose(raw, 0b101);
    /// assert_eq!(ptr.decompose(), (raw, 0b01));
    /// ```
    #[inline]
    pub fn compose(ptr: *mut T, tag: usize) -> Self {
        crate::assert_alignment::<T, N>();
        Self::new(crate::compose(ptr, tag, Self::TAG_BITS))
    }

    /// Returns the inner pointer *as is*, meaning any potential tag is **not**
    /// stripped.
    ///
    /// De-referencing the returned pointer results in undefined behaviour, if
    /// the pointer is still marked and even if the pointer itself points to a
    /// valid and live value.
    #[inline]
    pub const fn into_ptr(self) -> *mut T {
        self.inner
    }

    /// Cast to a pointer of another type.
    #[inline]
    pub fn cast<U>(self) -> MarkedPtr<U, N> {
        crate::assert_alignment::<U, N>();
        MarkedPtr { inner: self.inner.cast() }
    }

    /// Returns the integer representation of the pointer with its tag.
    #[inline]
    pub fn into_usize(self) -> usize {
        self.inner as usize
    }

    /// Returns `true` if the [`MarkedPtr`] is null.
    ///
    /// This is equivalent to calling `marked_ptr.decompose_ptr().is_null()`.
    #[inline]
    pub fn is_null(self) -> bool {
        self.decompose_ptr().is_null()
    }

    /// Clears the tag from `self` and returns the same pointer but stripped of
    /// its mark bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::ptr;
    ///
    /// type MarkedPtr = conquer_pointer::MarkedPtr<i32, 2>;
    ///
    /// let raw = &1 as *const i32 as *mut i32;
    /// let ptr = MarkedPtr::compose(raw, 0b11);
    /// assert_eq!(ptr.clear_tag().decompose(), (raw, 0));
    /// ```
    #[inline]
    pub fn clear_tag(self) -> Self {
        Self::new(self.decompose_ptr())
    }

    #[inline]
    pub fn split_tag(self) -> (Self, usize) {
        let (ptr, tag) = self.decompose();
        (Self::new(ptr), tag)
    }

    /// Clears the tag from `self` and replaces it with `tag`.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::ptr;
    ///
    /// type MarkedPtr = conquer_pointer::MarkedPtr<i32, 2>;
    ///
    /// let raw = &1 as *const i32 as *mut i32;
    /// let ptr = MarkedPtr::compose(raw, 0b11);
    /// assert_eq!(ptr.set_tag(0b10).decompose(), (raw, 0b10));
    /// ```
    #[inline]
    pub fn set_tag(self, tag: usize) -> Self {
        Self::compose(self.decompose_ptr(), tag)
    }

    /// Updates the tag of `self` with `func` and returns the pointer with the
    /// updated tag.
    ///
    /// # Examples
    ///
    /// ```
    /// type MarkedPtr = conquer_pointer::MarkedPtr<i32, 2>;
    ///
    /// let ptr = MarkedPtr::compose(&mut 1, 0b11);
    /// let ptr = ptr.update_tag(|tag| tag - 1);
    /// assert_eq!(ptr.decompose_tag(), 0b10);
    /// ```
    #[inline]
    pub fn update_tag(self, func: impl FnOnce(usize) -> usize) -> Self {
        let (ptr, tag) = self.decompose();
        Self::compose(ptr, func(tag))
    }

    /// Adds `value` to the current tag without regard for the previous value.
    ///
    /// This method does not perform any checks, so it may overflow the tag
    /// bits, result in a pointer to a different value, a null pointer or an
    /// unaligned pointer.
    #[inline]
    pub fn add_tag(self, value: usize) -> Self {
        Self::from_usize(self.into_usize() + value)
    }

    /// Subtracts `value` to the current tag without regard for the previous
    /// value.
    ///
    /// This method does not perform any checks, so it may underflow the tag
    /// bits, result in a pointer to a different value, a null pointer or an
    /// unaligned pointer.
    #[inline]
    pub fn sub_tag(self, value: usize) -> Self {
        Self::from_usize(self.into_usize() - value)
    }

    /// Decomposes the [`MarkedPtr`], returning the separated raw pointer and
    /// its tag.
    #[inline]
    pub fn decompose(self) -> (*mut T, usize) {
        crate::decompose::<T>(self.inner as usize, Self::TAG_BITS)
    }

    /// Decomposes the [`MarkedPtr`], returning only the separated raw pointer.
    #[inline]
    pub fn decompose_ptr(self) -> *mut T {
        crate::decompose_ptr::<T>(self.inner as usize, Self::TAG_BITS)
    }

    /// Decomposes the [`MarkedPtr`], returning only the separated tag value.
    #[inline]
    pub fn decompose_tag(self) -> usize {
        crate::decompose_tag::<T>(self.inner as usize, Self::TAG_BITS)
    }

    /// Decomposes the marked pointer, returning an optional reference and the
    /// separated tag.
    ///
    /// In case the pointer stripped of its tag is null, [`None`] is returned as
    /// part of the tuple. Otherwise, the reference is wrapped in a [`Some`].
    ///
    /// # Safety
    ///
    /// While this method and its mutable counterpart are useful for
    /// null-safety, it is important to note that this is still an unsafe
    /// operation because the returned value could be pointing to invalid
    /// memory.
    ///
    /// Additionally, the lifetime 'a returned is arbitrarily chosen and does
    /// not necessarily reflect the actual lifetime of the data.
    #[inline]
    pub unsafe fn decompose_ref<'a>(self) -> (Option<&'a T>, usize) {
        (self.as_ref(), self.decompose_tag())
    }

    /// Decomposes the marked pointer returning an optional mutable reference
    /// and the separated tag.
    ///
    /// In case the pointer stripped of its tag is null, [`None`] is returned as
    /// part of the tuple. Otherwise, the mutable reference is wrapped in a
    /// [`Some`].
    ///
    /// # Safety
    ///
    /// As with [`decompose_ref`][MarkedPtr::decompose_ref], this is unsafe
    /// because it cannot verify the validity of the returned pointer, nor can
    /// it ensure that the lifetime `'a` returned is indeed a valid lifetime for
    /// the contained data.
    #[inline]
    pub unsafe fn decompose_mut<'a>(self) -> (Option<&'a mut T>, usize) {
        (self.as_mut(), self.decompose_tag())
    }

    /// Decomposes the marked pointer, returning an optional reference and
    /// discarding the tag.
    ///
    /// # Safety
    ///
    /// The same caveats as with [`decompose_ref`][MarkedPtr::decompose_ref]
    /// apply for this method as well.
    #[inline]
    pub unsafe fn as_ref<'a>(self) -> Option<&'a T> {
        self.decompose_ptr().as_ref()
    }

    /// Decomposes the marked pointer, returning an optional mutable reference
    /// and discarding the tag.
    ///
    /// # Safety
    ///
    /// The same caveats as with [`decompose_mut`][MarkedPtr::decompose_mut]
    /// apply for this method as well.
    #[inline]
    pub unsafe fn as_mut<'a>(self) -> Option<&'a mut T> {
        self.decompose_ptr().as_mut()
    }    
}

/********** impl Default **************************************************************************/

impl<T, const N: usize> Default for MarkedPtr<T, N> {
    fn default() -> Self {
        Self::null()
    }
}

/********** impl From (*mut T) ********************************************************************/

impl<T, const N: usize> From<*mut T> for MarkedPtr<T, N> {
    #[inline]
    fn from(ptr: *mut T) -> Self {
        Self::new(ptr)
    }
}

/********** impl From (*const T) ******************************************************************/

impl<T, const N: usize> From<*const T> for MarkedPtr<T, N> {
    #[inline]
    fn from(ptr: *const T) -> Self {
        Self::new(ptr as *mut _)
    }
}

/********** impl From (&T) ************************************************************************/

impl<T, const N: usize> From<&T> for MarkedPtr<T, N> {
    #[inline]
    fn from(reference: &T) -> Self {
        Self::from(reference as *const _)
    }
}

/********** impl From (&mut T) ********************************************************************/

impl<T, const N: usize> From<&mut T> for MarkedPtr<T, N> {
    #[inline]
    fn from(reference: &mut T) -> Self {
        Self::new(reference)
    }
}

/********** impl From (NonNull) *******************************************************************/

impl<T, const N: usize> From<NonNull<T>> for MarkedPtr<T, N> {
    #[inline]
    fn from(non_null: NonNull<T>) -> Self {
        Self::new(non_null.as_ptr())
    }
}

/********** impl From (MarkedNonNull) *************************************************************/

impl<T, const N: usize> From<MarkedNonNull<T, N>> for MarkedPtr<T, N> {
    #[inline]
    fn from(non_null: MarkedNonNull<T, N>) -> Self {
        non_null.into_marked_ptr()
    }
}

/********** impl Debug ****************************************************************************/

impl<T, const N: usize> fmt::Debug for MarkedPtr<T, N> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MarkedPtr")
            .field("ptr", &self.decompose_ptr())
            .field("tag", &self.decompose_tag())
            .finish()
    }
}

/********** impl Pointer **************************************************************************/

impl<T, const N: usize> fmt::Pointer for MarkedPtr<T, N> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.decompose_ptr(), f)
    }
}

/********** impl PartialEq ************************************************************************/

impl<T, const N: usize> PartialEq for MarkedPtr<T, N> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

/********** impl PartialOrd ***********************************************************************/

impl<T, const N: usize> PartialOrd for MarkedPtr<T, N> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

/********** impl Eq *******************************************************************************/

impl<T, const N: usize> Eq for MarkedPtr<T, N> {}

/********** impl Ord ******************************************************************************/

impl<T, const N: usize> Ord for MarkedPtr<T, N> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

/********** impl Hash *****************************************************************************/

impl<T, const N: usize> Hash for MarkedPtr<T, N> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

#[cfg(test)]
mod tests {
    use core::ptr;

    type MarkedPtr = crate::MarkedPtr<i32, 2>;

    #[test]
    #[should_panic]
    fn illegal_type() {
        type InvMarkedPtr = crate::MarkedPtr<i32, 3>;
        let _ptr = InvMarkedPtr::compose(ptr::null_mut(), 0b111);
    }

    #[test]
    fn cast() {
        let ptr = MarkedPtr::compose(ptr::null_mut(), 0b11);
        let cast: crate::MarkedPtr<i64, 2> = ptr.cast();
        assert_eq!(cast.decompose(), (ptr::null_mut(), 0b11));
    }

    #[test]
    #[should_panic]
    fn illegal_cast() {
        let ptr = MarkedPtr::compose(ptr::null_mut(), 0b11);
        let _cast: crate::MarkedPtr<i8, 2> = ptr.cast();
    }

    #[test]
    fn from_usize() {
        let reference = &1 as *const i32 as usize;
        let ptr = MarkedPtr::from_usize(reference | 0b1);
        assert_eq!(ptr.into_usize(), reference | 0b1);
        assert_eq!(unsafe { ptr.decompose_ref() }, (Some(&1), 0b1))
    }

    #[test]
    fn from() {
        let mut val = 1;

        let from_ref = MarkedPtr::from(&val);
        let from_mut = MarkedPtr::from(&mut val);
        let from_const_ptr = MarkedPtr::from(&val as *const _);
        let from_mut_ptr = MarkedPtr::from(&mut val as *mut _);

        assert_eq!(from_ref, from_mut);
        assert_eq!(from_mut, from_const_ptr);
        assert_eq!(from_const_ptr, from_mut_ptr);
    }

    #[test]
    fn clear_tag() {
        let raw = &mut 1 as *mut i32;
        let ptr = MarkedPtr::compose(raw, 0);
        assert_eq!(ptr.clear_tag().into_ptr(), raw);
        assert_eq!(ptr.clear_tag().decompose(), (raw, 0));

        let ptr = MarkedPtr::compose(raw, 0b11);
        assert_eq!(ptr.clear_tag().into_ptr(), raw);
        assert_eq!(ptr.clear_tag().decompose(), (raw, 0));
    }

    #[test]
    fn with_tag() {
        let raw = &mut 1 as *mut i32;
        let unmarked = MarkedPtr::compose(raw, 0);
        let marked_ptr = MarkedPtr::compose(raw, 0b11);

        assert_eq!(unmarked.set_tag(0b1), MarkedPtr::compose(raw, 0b1));
        assert_eq!(marked_ptr.set_tag(0b1), MarkedPtr::compose(raw, 0b1));
        assert_eq!(unmarked.set_tag(0b101), MarkedPtr::compose(raw, 0b1));
        assert_eq!(marked_ptr.set_tag(0b101), MarkedPtr::compose(raw, 0b1));
        assert_eq!(unmarked.set_tag(0).into_ptr(), raw);
        assert_eq!(marked_ptr.set_tag(0).into_ptr(), raw);
    }

    #[test]
    fn decompose() {
        assert_eq!(MarkedPtr::compose(ptr::null_mut(), 0).decompose(), (ptr::null_mut(), 0));
        assert_eq!(MarkedPtr::compose(ptr::null_mut(), 0b11).decompose(), (ptr::null_mut(), 0b11));
        assert_eq!(MarkedPtr::compose(ptr::null_mut(), 0b100).decompose(), (ptr::null_mut(), 0));

        let ptr = &mut 0xBEEF as *mut i32;
        assert_eq!(MarkedPtr::compose(ptr, 0).decompose(), (ptr, 0));
        assert_eq!(MarkedPtr::compose(ptr, 0b11).decompose(), (ptr, 0b11));
        assert_eq!(MarkedPtr::compose(ptr, 0b100).decompose(), (ptr, 0));
    }

    #[test]
    fn decompose_ref() {
        let null = MarkedPtr::null();
        assert_eq!(unsafe { null.decompose_ref() }, (None, 0));
        let marked_null = MarkedPtr::compose(ptr::null_mut(), 0b11);
        assert_eq!(unsafe { marked_null.decompose_ref() }, (None, 0b11));
        let ptr = MarkedPtr::compose(&mut 1, 0b01);
        assert_eq!(unsafe { ptr.decompose_ref() }, (Some(&1), 0b01));
    }

    #[test]
    fn decompose_mut() {
        let ptr = MarkedPtr::compose(&mut 1, 0b01);
        assert_eq!(unsafe { ptr.decompose_mut() }, (Some(&mut 1), 0b01));
    }
}
