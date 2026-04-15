// 3×5 bitmap text renderer — blindly fast. Each glyph is a 15-bit bitmap
// held in a storage buffer indexed by ASCII code. The fragment shader does
// one buffer fetch and one bit extract per pixel.

struct TextParams {
    origin_px: vec2f,   // top-left of text in screen pixels
    scale: f32,         // pixel scale (e.g. 2.0 = each bitmap pixel → 2×2 screen pixels)
    _pad0: f32,
    color: vec4f,       // RGBA when a glyph pixel is set
    screen_size: vec2f, // window dims in pixels
    text_len: u32,      // number of active characters in `text`
    _pad1: u32,
}

@group(0) @binding(0) var<uniform> params: TextParams;
@group(0) @binding(1) var<storage, read> text: array<u32>;
@group(0) @binding(2) var<storage, read> glyphs: array<u32>;

struct VSOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

// Fullscreen triangle-strip quad — fragment shader early-discards outside the
// text rect so unneeded pixels cost one float compare and one discard.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VSOut {
    var out: VSOut;
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    out.pos = vec4f(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2f(x, y);
    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4f {
    let px = in.uv * params.screen_size;
    let local = px - params.origin_px;

    let char_w = 4.0 * params.scale;  // 3 pixel columns + 1 gap
    let char_h = 5.0 * params.scale;

    if local.x < 0.0 || local.y < 0.0 || local.y >= char_h {
        discard;
    }
    let char_idx = u32(floor(local.x / char_w));
    if char_idx >= params.text_len {
        discard;
    }

    let within = vec2f(local.x - f32(char_idx) * char_w, local.y);
    let col = u32(floor(within.x / params.scale));
    let row = u32(floor(within.y / params.scale));
    if col >= 3u || row >= 5u {
        discard;
    }

    let c = text[char_idx];
    // Bounds-check the glyph index; anything outside the table → blank.
    if c >= arrayLength(&glyphs) {
        discard;
    }
    let glyph = glyphs[c];
    let bit = row * 3u + col;
    if ((glyph >> bit) & 1u) == 0u {
        discard;
    }
    return params.color;
}
