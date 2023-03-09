struct Globals {
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
    @location(2) rotation: f32,
    @location(3) mirrored: u32,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var mirroring: vec2<f32>;
    if instance.mirrored != 0u {
        mirroring = vec2<f32>(-1.0, 1.0);
    } else {
        mirroring = vec2<f32>(1.0, 1.0);
    }

    let sin = sin(instance.rotation);
    let cos = cos(instance.rotation);
    let rotation = mat2x2<f32>(cos, -sin, sin, cos);

    let local_position = ((vertex.position * mirroring) * rotation) + instance.offset;
    let world_position = (local_position - globals.offset) * globals.zoom;
    let clip_position = (world_position / globals.resolution) * vec2<f32>(2.0, 2.0);

    var result: VertexOutput;
    result.position = vec4<f32>(clip_position, 0.0, 1.0);
    result.color = instance.color;
    return result;
}

@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}
