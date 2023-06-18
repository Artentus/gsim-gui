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
    @location(2) rotation: u32,
    @location(3) mirrored: u32,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var vertex_position = vertex.position;

    if instance.mirrored != 0u {
        vertex_position.x = -vertex_position.x;
    }

    switch instance.rotation {
        case 1u: {
            vertex_position = vec2<f32>(vertex_position.y, -vertex_position.x);
        }
        case 2u: {
            vertex_position = vec2<f32>(-vertex_position.x, -vertex_position.y);
        }
        case 3u: {
            vertex_position = vec2<f32>(-vertex_position.y, vertex_position.x);
        }
        default: {}
    }

    let local_position = vertex_position + instance.offset;
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
