struct Globals {
    color: vec4<f32>,
    selected_color: vec4<f32>,
    resolution: vec2<f32>,
    offset: vec2<f32>,
    zoom: f32,
};

@group(0)
@binding(0)
var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) selected: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
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
    result.color = color;
    return result;
}

@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}
