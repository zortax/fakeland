//! Handles of reference images and the sources that resolve them.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::image::{Image, ImageView};

/// A handle that names a stored reference image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImageHandle(pub u64);

/// Resolves image handles to images.
pub trait ImageSource {
    /// Returns the image of a handle, or `None` when the handle names no image.
    fn image(&self, handle: ImageHandle) -> Option<ImageView<'_>>;
}

/// A source without images, for predicates that name none.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoImages;

impl ImageSource for NoImages {
    fn image(&self, _handle: ImageHandle) -> Option<ImageView<'_>> {
        None
    }
}

impl ImageSource for HashMap<ImageHandle, Image> {
    fn image(&self, handle: ImageHandle) -> Option<ImageView<'_>> {
        self.get(&handle).map(Image::view)
    }
}

/// An owned set of reference images with generated handles.
#[derive(Debug, Default)]
pub struct ImageStore {
    images: HashMap<ImageHandle, Image>,
    next: u64,
}

impl ImageStore {
    /// Builds an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores an image under a new handle.
    pub fn insert(&mut self, image: Image) -> ImageHandle {
        let handle = ImageHandle(self.next);
        self.next += 1;
        self.images.insert(handle, image);
        handle
    }

    /// Stores an image under a chosen handle and returns the image it replaces.
    pub fn insert_at(&mut self, handle: ImageHandle, image: Image) -> Option<Image> {
        self.next = self.next.max(handle.0.saturating_add(1));
        self.images.insert(handle, image)
    }

    /// Returns the image of a handle.
    pub fn get(&self, handle: ImageHandle) -> Option<&Image> {
        self.images.get(&handle)
    }

    /// Removes and returns the image of a handle.
    pub fn remove(&mut self, handle: ImageHandle) -> Option<Image> {
        self.images.remove(&handle)
    }

    /// Returns the number of stored images.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Returns true when the store holds no image.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

impl ImageSource for ImageStore {
    fn image(&self, handle: ImageHandle) -> Option<ImageView<'_>> {
        self.get(handle).map(Image::view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_do_not_collide() {
        let mut s = ImageStore::new();
        s.insert_at(ImageHandle(5), Image::new(1, 1));
        let h = s.insert(Image::new(2, 2));
        assert_eq!(h, ImageHandle(6));
        assert_eq!(s.image(h).map(|v| v.width()), Some(2));
        assert_eq!(s.len(), 2);
        assert!(s.remove(ImageHandle(5)).is_some());
        assert!(s.image(ImageHandle(5)).is_none());
        assert!(NoImages.image(h).is_none());
    }
}
