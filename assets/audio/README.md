# RustCraft original audio

The sound assets are original, deterministic synthesized samples. They are
generated once in memory, not downloaded recordings or copied game audio.

| Field | Provenance |
| --- | --- |
| Creator | RustCraft project; agent-authored DSP implementation |
| Source | `src/app/audio/synthesis.rs` and `src/app/audio/catalog.rs` |
| License | Project MIT license (`LICENSE`); no external asset attribution required |
| Material | Seeded xorshift noise, low-pass filters, short damped sine resonances, periodic envelopes |
| Seeds | Action index + 1; environment index + 101, multiplied by `0x9e3779b9` |
| Processing | Envelope/peak normalization; environmental DC removal and one-second end/start overlap |
| Format | Mono f32, 48 kHz source rate; linearly resampled to the output device |
| Redistribution | Source-generated project assets; no proprietary/reference-site/game recordings |

The bank has 29 short action textures and four environmental beds
(wind 24 s, surf 31 s, freshwater 29 s, cave 37 s). Underwater is a smooth
filtering/listening state, not a separate recording. Samples are preloaded;
callbacks do no decoding, filesystem reads, or procedural world generation.
Tests measure the actual bank against the 32 MiB limit and check finite samples,
peak/RMS, silence at action endpoints, and ambience seam continuity. These tests
do not certify realism, pleasantness, repetition, or audible playback.

CPAL 0.18.2 provides native playback under Apache-2.0; it is a code dependency,
not a source of sound assets. Ordinary tests and graphics smoke launches use
silent/fake output and never open a device.
