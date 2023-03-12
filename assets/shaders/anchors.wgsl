struct Globals {
    input_color: vec4<f32>,
    output_color: vec4<f32>,
    bidirectional_color: vec4<f32>,
    resolution: vec2<f32>,
    offset: vec2<f32>,
    zoom: f32,
};

@group(0)
@binding(0)
var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct InstanceInput {
    @location(1) offset: vec2<f32>,
    @location(2) kind: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let local_position = vertex.position + instance.offset;
    let world_position = (local_position - globals.offset) * globals.zoom;
    let clip_position = (world_position / globals.resolution) * vec2<f32>(2.0, 2.0);

    var color: vec4<f32>;
    switch instance.kind {
        default: {
            color = globals.input_color;
        }
        case 1u: {
            color = globals.output_color;
        }
        case 2u: {
            color = globals.bidirectional_color;
        }
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
