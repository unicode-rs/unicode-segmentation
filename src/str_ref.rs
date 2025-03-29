use core::ops::Deref;

/// This trait is a common abstraction for &str and &mut str.
/// It combines all the string operations that this crate relies on, such that our code can be written generically and applied to both.
pub trait StrRef: Deref<Target = str> + Default {
    fn split_at(self, idx: usize) -> (Self, Self);
}

impl StrRef for &str {
    fn split_at(self, idx: usize) -> (Self, Self) {
        self.split_at(idx)
    }
}

impl StrRef for &mut str {
    fn split_at(self, idx: usize) -> (Self, Self) {
        self.split_at_mut(idx)
    }
}
