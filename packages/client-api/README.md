# Client API

`types.ts` is a hand-written mirror of `honkoku-core::model`. Generation from Rust is planned. Keep field names, optional/null values, RFC3339 timestamp strings, language maps, and open status strings synchronized when changing either side. Unknown fields are flattened into the containing object.

`invoke.ts` exposes the six typed Tauri read commands. Rejections carry `{ kind, message }`. The Rust layer owns remote access and the ten-minute public cache.

OCR engine results accept plain strings and structured records with `text`, `createdAt`, and `textBlocks`; the public entry fixture contains the structured form. Block metadata remains unmodified JSON.
