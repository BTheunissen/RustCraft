//! Original jagged fracture branches, shared with shader generation and CPU checks.

pub(super) struct CrackSegment {
    pub(super) start: [f32; 2],
    pub(super) end: [f32; 2],
    pub(super) onset: f32,
}

const fn segment(start: [f32; 2], end: [f32; 2], onset: f32) -> CrackSegment {
    CrackSegment { start, end, onset }
}

pub(super) const SEGMENTS: [CrackSegment; 24] = [
    segment([0.08, 0.15], [0.24, 0.29], 0.12),
    segment([0.24, 0.29], [0.31, 0.27], 0.06),
    segment([0.31, 0.27], [0.44, 0.47], 0.0),
    segment([0.44, 0.47], [0.54, 0.54], 0.0),
    segment([0.54, 0.54], [0.63, 0.70], 0.04),
    segment([0.63, 0.70], [0.84, 0.87], 0.16),
    segment([0.44, 0.47], [0.30, 0.53], 0.22),
    segment([0.30, 0.53], [0.22, 0.72], 0.30),
    segment([0.22, 0.72], [0.09, 0.82], 0.44),
    segment([0.54, 0.54], [0.72, 0.43], 0.26),
    segment([0.72, 0.43], [0.83, 0.45], 0.34),
    segment([0.83, 0.45], [0.94, 0.29], 0.50),
    segment([0.31, 0.27], [0.36, 0.12], 0.36),
    segment([0.36, 0.12], [0.32, 0.04], 0.58),
    segment([0.63, 0.70], [0.61, 0.86], 0.40),
    segment([0.61, 0.86], [0.68, 0.96], 0.64),
    segment([0.44, 0.47], [0.49, 0.30], 0.46),
    segment([0.49, 0.30], [0.63, 0.20], 0.54),
    segment([0.63, 0.20], [0.70, 0.08], 0.72),
    segment([0.72, 0.43], [0.75, 0.61], 0.60),
    segment([0.75, 0.61], [0.90, 0.70], 0.78),
    segment([0.22, 0.72], [0.35, 0.84], 0.66),
    segment([0.24, 0.29], [0.12, 0.37], 0.70),
    segment([0.49, 0.30], [0.52, 0.16], 0.80),
];

pub(super) fn shader_segments() -> String {
    let segments: String = SEGMENTS
        .iter()
        .map(|segment| {
            format!(
                "CrackSegment(vec2<f32>({}, {}), vec2<f32>({}, {}), {}),\n",
                segment.start[0], segment.start[1], segment.end[0], segment.end[1], segment.onset,
            )
        })
        .collect();
    format!(
        "struct CrackSegment {{ start: vec2<f32>, end: vec2<f32>, onset: f32, }};\n\
         const CRACK_SEGMENTS = array<CrackSegment, {}>(\n{segments});\n",
        SEGMENTS.len(),
    )
}
