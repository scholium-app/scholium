//! Exact pixel tile reuse. Every response describes a complete scene, not a delta.
use serde_json::{Value, json};
use std::{collections::HashMap, error::Error, fs};

const TILE_PIXELS: usize = 128;
type Key = (usize, usize, usize);

#[derive(Default)]
pub(crate) struct Tiles {
    next: u64,
    previous: HashMap<Key, Tile>,
    current: HashMap<Key, Tile>,
}

struct Tile {
    id: u64,
    size: [usize; 2],
    bytes: Vec<u8>,
}

impl Tiles {
    pub fn page(
        &mut self,
        page: usize,
        image: tiny_skia::Pixmap,
    ) -> Result<Vec<Value>, Box<dyn Error>> {
        let width = image.width() as usize;
        let height = image.height() as usize;
        // Use the same demultiplication as PNG export for identical edge pixels.
        let rgba = image.take_demultiplied();
        let mut output = Vec::new();
        for y in (0..height).step_by(TILE_PIXELS) {
            for x in (0..width).step_by(TILE_PIXELS) {
                let size = [TILE_PIXELS.min(width - x), TILE_PIXELS.min(height - y)];
                let bytes = (y..y + size[1])
                    .flat_map(|row| {
                        rgba[(row * width + x) * 4..(row * width + x + size[0]) * 4]
                            .iter()
                            .copied()
                    })
                    .collect();
                output.push(self.tile((page, x, y), size, bytes)?);
            }
        }
        Ok(output)
    }

    fn tile(
        &mut self,
        key: Key,
        size: [usize; 2],
        bytes: Vec<u8>,
    ) -> Result<Value, Box<dyn Error>> {
        let old = self.previous.remove(&key);
        let tile = if old
            .as_ref()
            .is_some_and(|t| t.size == size && t.bytes == bytes)
        {
            old.expect("checked existing tile")
        } else {
            if let Some(old) = old {
                let _ = fs::remove_file(format!("/work/tile-{}.rgba", old.id));
            }
            self.next += 1;
            fs::write(format!("/work/tile-{}.rgba", self.next), &bytes)?;
            Tile {
                id: self.next,
                size,
                bytes,
            }
        };
        let value = json!({"id": tile.id, "x": key.1, "y": key.2,
            "width": size[0], "height": size[1]});
        self.current.insert(key, tile);
        Ok(value)
    }

    pub fn finish(&mut self) {
        for tile in self.previous.values() {
            let _ = fs::remove_file(format!("/work/tile-{}.rgba", tile.id));
        }
        self.previous = std::mem::take(&mut self.current);
    }
}
