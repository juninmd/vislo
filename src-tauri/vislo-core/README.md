# vislo-core

Vislo's image decoding, without the UI: which files are images (`formats`), decoding every supported format
(`decode`), EXIF and metadata (`exif_info`, `metadata`), thumbnails (`thumbs`) and resizing (`resize`).
The Vislo app depends on it, and other Rust programs can use it to show the same images.

```toml
[dependencies]
vislo-core = { git = "https://github.com/juninmd/vislo" }
```

```rust
use std::path::Path;
use vislo_core::{decode, formats};

let path = Path::new("photo.psd");
if formats::is_image(path) {
    let img = decode::load_fit(path, 1920, 1080)?; // upright image::DynamicImage; can exceed the box, so shrink it with resize::fit
}
```

Tests run with the app's: `cargo test --manifest-path src-tauri/Cargo.toml --workspace`.
