//! Generic IndexSpace container for uniform access.

use std::ops::Index;

/// A container for items in an index space, supporting O(1) access by index.
#[derive(Debug, Clone)]
pub struct IndexSpace<T>(Vec<T>);

impl<T> Default for IndexSpace<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> IndexSpace<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, item: T) -> u32 {
        let idx = self.0.len() as u32;
        self.0.push(item);
        idx
    }

    pub fn get(&self, idx: u32) -> Option<&T> {
        self.0.get(idx as usize)
    }

    pub fn get_mut(&mut self, idx: u32) -> Option<&mut T> {
        self.0.get_mut(idx as usize)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.0.iter().enumerate().map(|(i, t)| (i as u32, t))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> Index<u32> for IndexSpace<T> {
    type Output = T;

    fn index(&self, idx: u32) -> &Self::Output {
        &self.0[idx as usize]
    }
}
