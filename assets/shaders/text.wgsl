struct Globals {
    color: vec4<f32>,
    selected_color: vec4<f32>,
    resolution: vec2<f32>,
    offset: vec2<f32>,
    zoom: f32,
    px_range: f32,
};

@group(0)
@binding(0)
var<uniform> globals: Globals;

@group(0)
@binding(1)
var atlas: texture_2d<f32>;

@group(0)
@binding(2)
var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) selected: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput) -> VertexOutput {
    let world_position = (vertex.position - globals.offset) * globals.zoom;
    let clip_position = (world_position / globals.resolution) * vec2<f32>(2.0, 2.0);

    var color: vec4<f32>;
    if vertex.selected != 0u {
        color = globals.selected_color;
    } else {
        color = globals.color;
    }

    var result: VertexOutput;
    result.position = vec4<f32>(clip_position, 0.0, 1.0);
    result.uv = vertex.uv;
    result.color = color;
    return result;
}

fn median(dist: vec3<f32>) -> f32 {
    return max(min(dist.r, dist.g), min(max(dist.r, dist.g), dist.b));
}

@fragment
fn fs_main(@location(0) uv: vec2<f32>, @location(1) color: vec4<f32>) -> @location(0) vec4<f32> {
    let dist = median(textureSample(atlas, atlas_sampler, uv).rgb);
    let px_dist = (globals.px_range * (dist - 0.5)) + 0.5;
    let opacity = clamp(px_dist, 0.0, 1.0);

    var out_color = color;
    out_color.a *= opacity;
    return out_color;
}
