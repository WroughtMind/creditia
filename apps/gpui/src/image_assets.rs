//! Pair decoder-cache eviction with GPU sprite-atlas eviction. Removing only
//! ImageSource's asset leaves atlas allocations alive in GPUI 0.3.11.
use gpui_kit::{App, Global, Image, ImageSource, RenderImage, Window};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

struct Entry {
    // Weak retains allocation identity, without retaining encoded source bytes.
    source: Weak<Image>,
    source_id: u64,
    decoded: Arc<RenderImage>,
}
#[derive(Default)]
struct Images {
    entries: HashMap<usize, Entry>,
    atlas_releases: u64,
}
impl Global for Images {}

impl Images {
    fn record(
        &mut self,
        source: &Arc<Image>,
        decoded: Arc<RenderImage>,
    ) -> Option<Arc<RenderImage>> {
        let old = self.entries.insert(
            Arc::as_ptr(source) as usize,
            Entry {
                source: Arc::downgrade(source),
                source_id: source.id(),
                decoded,
            },
        );
        old.filter(|old| {
            !self
                .entries
                .values()
                .any(|entry| entry.decoded.id == old.decoded.id)
        })
        .map(|old| old.decoded)
    }

    fn remove(&mut self, source: &Arc<Image>) -> (bool, Option<Arc<RenderImage>>) {
        let key = Arc::as_ptr(source) as usize;
        let old = if self
            .entries
            .get(&key)
            .is_some_and(|entry| Weak::ptr_eq(&entry.source, &Arc::downgrade(source)))
        {
            self.entries.remove(&key)
        } else {
            None
        };
        // Separate Image allocations with equal content share GPUI's loader.
        let remove_loader = !self
            .entries
            .values()
            .any(|entry| entry.source_id == source.id());
        let decoded = old
            .filter(|old| {
                !self
                    .entries
                    .values()
                    .any(|entry| entry.decoded.id == old.decoded.id)
            })
            .map(|old| old.decoded);
        (remove_loader, decoded)
    }
}

fn retire(decoded: Arc<RenderImage>, cx: &mut App) {
    // A render/paint callback temporarily removes the current Window from App.
    // Deferred effects run after it returns, so this reaches every window.
    cx.defer(move |cx| {
        // An equal image may have been reused in the same effect cycle.
        if cx.try_global::<Images>().is_some_and(|images| {
            images
                .entries
                .values()
                .any(|entry| entry.decoded.id == decoded.id)
        }) {
            return;
        }
        cx.drop_image(decoded, None);
        cx.default_global::<Images>().atlas_releases += 1;
    });
}

/// Call wherever a visible image is preloaded. This is the normal decoder, with
/// its result retained only until the matching release of the source identity.
pub fn track(image: &Arc<Image>, window: &mut Window, cx: &mut App) -> Option<Arc<RenderImage>> {
    let decoded = image.clone().get_render_image(window, cx)?;
    // Normal repaint stays O(1), without mutating a GPUI global or walking the
    // entire wall. The weak source in the entry prevents pointer reuse.
    if cx.try_global::<Images>().is_some_and(|images| {
        images
            .entries
            .get(&(Arc::as_ptr(image) as usize))
            .is_some_and(|entry| entry.decoded.id == decoded.id)
    }) {
        return Some(decoded);
    }
    if let Some(old) = cx.default_global::<Images>().record(image, decoded.clone()) {
        retire(old, cx);
    }
    Some(decoded)
}

/// Call only after the UI's last owner of this exact Image Arc is finished.
/// Distinct Image Arcs sharing the same decoded image are counted separately.
pub fn release(image: Arc<Image>, cx: &mut App) {
    let (remove_loader, decoded) = cx.default_global::<Images>().remove(&image);
    if remove_loader {
        ImageSource::Image(image).remove_asset(cx);
    }
    if let Some(decoded) = decoded {
        retire(decoded, cx);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub tracked_sources: usize,
    pub decoded_images: usize,
    pub decoded_bytes: usize,
    /// Completed calls to GPUI's explicit atlas removal, not estimated bytes.
    pub atlas_releases: u64,
}

pub fn stats(cx: &App) -> Stats {
    let Some(images) = cx.try_global::<Images>() else {
        return Stats::default();
    };
    let mut unique = HashMap::new();
    for entry in images.entries.values() {
        unique.entry(entry.decoded.id.0).or_insert_with(|| {
            (0..entry.decoded.frame_count())
                .map(|index| entry.decoded.as_bytes(index).map_or(0, |bytes| bytes.len()))
                .sum::<usize>()
        });
    }
    Stats {
        tracked_sources: images.entries.len(),
        decoded_images: unique.len(),
        decoded_bytes: unique.values().sum(),
        atlas_releases: images.atlas_releases,
    }
}

#[cfg(test)]
mod tests {
    use super::Images;
    use gpui_kit::{Image, ImageFormat, SvgRenderer};
    use std::sync::Arc;

    fn source() -> Arc<Image> {
        Arc::new(Image::from_bytes(ImageFormat::Svg,
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4"><rect width="8" height="4" fill="#f00"/></svg>"##.to_vec()))
    }

    #[test]
    fn equal_sources_keep_the_shared_loader_and_atlas_until_the_last_release() {
        let a = source();
        let b = source();
        assert!(!Arc::ptr_eq(&a, &b));
        let decoded = a.to_image_data(SvgRenderer::new(Arc::new(()))).unwrap();
        let mut images = Images::default();
        assert!(images.record(&a, decoded.clone()).is_none());
        assert!(images.record(&b, decoded.clone()).is_none());
        assert_eq!(
            Arc::strong_count(&a),
            1,
            "tracking must not retain source bytes"
        );
        let (loader, atlas) = images.remove(&a);
        assert!(!loader && atlas.is_none());
        let (loader, atlas) = images.remove(&b);
        assert!(loader && Arc::ptr_eq(&atlas.unwrap(), &decoded));
        assert!(images.entries.is_empty());
    }

    #[test]
    fn replacing_a_decoded_generation_retires_only_the_old_image() {
        let a = source();
        let first = a.to_image_data(SvgRenderer::new(Arc::new(()))).unwrap();
        let next = a.to_image_data(SvgRenderer::new(Arc::new(()))).unwrap();
        assert_ne!(first.id, next.id);
        let mut images = Images::default();
        images.record(&a, first.clone());
        assert!(Arc::ptr_eq(
            &images.record(&a, next.clone()).unwrap(),
            &first
        ));
        assert!(images.record(&a, next.clone()).is_none());
        assert!(Arc::ptr_eq(&images.remove(&a).1.unwrap(), &next));
        assert!(
            images.remove(&a).1.is_none(),
            "double release cannot free a reused atlas slot"
        );
    }
}
