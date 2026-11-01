//! A table of equal-width rows in one flat vector: `rows[i][j]` is `flat[i * width + j]`. Used
//! for goods-indexed rows whose width, the goods count, is a pack value.

use std::ops::{Index, IndexMut};
use std::slice::{ChunksExact, ChunksExactMut};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rows<T> {
    data: Vec<T>,
    width: usize,
}

impl<T: Clone> Rows<T> {
    /// No rows of `width`.
    pub fn new(width: usize) -> Self {
        Self {
            data: Vec::new(),
            width,
        }
    }

    /// `len` rows of `width`, every entry `value`.
    pub fn filled(len: usize, width: usize, value: T) -> Self {
        Self {
            data: vec![value; len * width],
            width,
        }
    }

    /// Panics unless `data` holds whole rows of `width`.
    pub fn from_flat(data: Vec<T>, width: usize) -> Self {
        assert!(
            width > 0 || data.is_empty(),
            "{} entries in rows of width 0",
            data.len()
        );
        assert!(
            width == 0 || data.len().is_multiple_of(width),
            "{} entries are not whole rows of {width}",
            data.len()
        );
        Self { data, width }
    }

    /// Rows of `width` from an iterator of rows, each of that width.
    pub fn from_rows<R: AsRef<[T]>>(width: usize, rows: impl IntoIterator<Item = R>) -> Self {
        let mut out = Self::new(width);
        for r in rows {
            out.push(r.as_ref());
        }
        out
    }

    pub fn push(&mut self, row: &[T]) {
        assert_eq!(row.len(), self.width, "row width");
        self.data.extend_from_slice(row);
    }
}

impl<T> Rows<T> {
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn len(&self) -> usize {
        self.data.len().checked_div(self.width).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn iter(&self) -> ChunksExact<'_, T> {
        self.data.chunks_exact(self.width.max(1))
    }

    pub fn iter_mut(&mut self) -> ChunksExactMut<'_, T> {
        self.data.chunks_exact_mut(self.width.max(1))
    }

    pub fn as_flat(&self) -> &[T] {
        &self.data
    }

    pub fn as_flat_mut(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn into_flat(self) -> Vec<T> {
        self.data
    }
}

impl<T> Index<usize> for Rows<T> {
    type Output = [T];

    fn index(&self, i: usize) -> &[T] {
        &self.data[i * self.width..(i + 1) * self.width]
    }
}

impl<T> IndexMut<usize> for Rows<T> {
    fn index_mut(&mut self, i: usize) -> &mut [T] {
        &mut self.data[i * self.width..(i + 1) * self.width]
    }
}

impl<'a, T> IntoIterator for &'a Rows<T> {
    type Item = &'a [T];
    type IntoIter = ChunksExact<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Rows<T> {
    type Item = &'a mut [T];
    type IntoIter = ChunksExactMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_index_the_flat_vector() {
        let mut r = Rows::from_flat((0..6).collect::<Vec<u32>>(), 3);
        assert_eq!((r.len(), r.width()), (2, 3));
        assert_eq!(&r[1], &[3, 4, 5]);
        r[0][2] = 9;
        r.push(&[6, 7, 8]);
        assert_eq!(r.as_flat(), &[0, 1, 9, 3, 4, 5, 6, 7, 8]);
        assert_eq!(r.iter().map(|x| x[0]).collect::<Vec<_>>(), [0, 3, 6]);
    }

    #[test]
    fn empty_rows_have_no_length() {
        let r: Rows<f64> = Rows::default();
        assert_eq!((r.len(), r.iter().count()), (0, 0));
    }

    #[test]
    #[should_panic(expected = "whole rows")]
    fn ragged_data_is_refused() {
        Rows::from_flat(vec![0.0; 5], 2);
    }
}
