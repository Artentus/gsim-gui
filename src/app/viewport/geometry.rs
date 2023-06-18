use super::{BufferUsages, Device, StaticBuffer, Vec2f, LOGICAL_PIXEL_SIZE};
use bytemuck::{Pod, Zeroable};
use lyon::math::*;
use lyon::path::*;
use lyon::tessellation::*;

trait BuilderExt {
    fn circle(&mut self, center: Point, radius: f32);
}

impl BuilderExt for lyon::path::path::Builder {
    fn circle(&mut self, center: Point, radius: f32) {
        const CTRL_POS: f32 = 0.552284749831;

        self.begin(center + vector(0.0, -1.0) * radius);
        self.cubic_bezier_to(
            center + vector(-CTRL_POS, -1.0) * radius,
            center + vector(-1.0, -CTRL_POS) * radius,
            center + vector(-1.0, 0.0) * radius,
        );
        self.cubic_bezier_to(
            center + vector(-1.0, CTRL_POS) * radius,
            center + vector(-CTRL_POS, 1.0) * radius,
            center + vector(0.0, 1.0) * radius,
        );
        self.cubic_bezier_to(
            center + vector(CTRL_POS, 1.0) * radius,
            center + vector(1.0, CTRL_POS) * radius,
            center + vector(1.0, 0.0) * radius,
        );
        self.cubic_bezier_to(
            center + vector(1.0, -CTRL_POS) * radius,
            center + vector(CTRL_POS, -1.0) * radius,
            center + vector(0.0, -1.0) * radius,
        );
        self.close();
    }
}

const GEOMETRY_TOLERANCE: f32 = LOGICAL_PIXEL_SIZE / 16.0;

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(super) struct Vertex {
    position: Vec2f,
}

pub(super) struct Geometry {
    vertices: StaticBuffer<Vertex>,
    indices: StaticBuffer<u16>,
}

impl Geometry {
    #[inline]
    pub(super) fn vertices(&self) -> &StaticBuffer<Vertex> {
        &self.vertices
    }

    #[inline]
    pub(super) fn indices(&self) -> &StaticBuffer<u16> {
        &self.indices
    }
}

macro_rules! geometry {
    ($device:expr, $stroke:expr, $fill:expr, $label:literal) => {
        (
            Geometry {
                vertices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " stroke vertices")),
                    BufferUsages::VERTEX,
                    &$stroke.vertices,
                ),
                indices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " stroke indices")),
                    BufferUsages::INDEX,
                    &$stroke.indices,
                ),
            },
            Geometry {
                vertices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " fill vertices")),
                    BufferUsages::VERTEX,
                    &$fill.vertices,
                ),
                indices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " fill indices")),
                    BufferUsages::INDEX,
                    &$fill.indices,
                ),
            },
        )
    };
}

fn build_and_gate_geometry(device: &Device) -> (Geometry, Geometry) {
    const CIRCLE_ARC_CTRL_POS: f32 = 0.552284749831;

    let mut builder = Path::builder();
    builder.begin(point(-2.0, -2.0));
    builder.line_to(point(-2.0, 0.0));
    builder.cubic_bezier_to(
        point(-2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        point(-CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        point(0.0, 2.0),
    );
    builder.cubic_bezier_to(
        point(CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        point(2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        point(2.0, 0.0),
    );
    builder.line_to(point(2.0, -2.0));
    builder.close();
    let path = builder.build();

    let mut stroke_geometry = VertexBuffers::new();
    let mut stroke_tessellator = StrokeTessellator::new();
    stroke_tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(2.0 * LOGICAL_PIXEL_SIZE)
                .with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut stroke_geometry, |v: StrokeVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    let mut fill_geometry = VertexBuffers::new();
    let mut fill_tessellator = FillTessellator::new();
    fill_tessellator
        .tessellate_path(
            &path,
            &FillOptions::DEFAULT.with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut fill_geometry, |v: FillVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    geometry!(device, stroke_geometry, fill_geometry, "AND gate")
}

fn build_or_gate_geometry(device: &Device) -> (Geometry, Geometry) {
    let mut builder = Path::builder();
    builder.begin(point(-2.0, -2.35));
    builder.quadratic_bezier_to(point(0.0, -1.35), point(2.0, -2.35));
    builder.line_to(point(2.0, -1.8));
    builder.quadratic_bezier_to(point(2.0, 1.0), point(0.0, 2.0));
    builder.quadratic_bezier_to(point(-2.0, 1.0), point(-2.0, -1.8));
    builder.line_to(point(-2.0, -2.35));
    builder.close();
    let path = builder.build();

    let mut stroke_geometry = VertexBuffers::new();
    let mut stroke_tessellator = StrokeTessellator::new();
    stroke_tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(2.0 * LOGICAL_PIXEL_SIZE)
                .with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut stroke_geometry, |v: StrokeVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    let mut fill_geometry = VertexBuffers::new();
    let mut fill_tessellator = FillTessellator::new();
    fill_tessellator
        .tessellate_path(
            &path,
            &FillOptions::DEFAULT.with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut fill_geometry, |v: FillVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    geometry!(device, stroke_geometry, fill_geometry, "OR gate")
}

fn build_xor_gate_geometry(device: &Device) -> (Geometry, Geometry) {
    let mut builder = Path::builder();
    builder.begin(point(-2.0, -1.8));
    builder.quadratic_bezier_to(point(0.0, -0.8), point(2.0, -1.8));
    builder.quadratic_bezier_to(point(2.0, 1.0), point(0.0, 2.0));
    builder.quadratic_bezier_to(point(-2.0, 1.0), point(-2.0, -1.8));
    builder.close();
    builder.begin(point(-2.0, -2.35));
    builder.quadratic_bezier_to(point(0.0, -1.35), point(2.0, -2.35));
    builder.end(false);
    let path = builder.build();

    let mut stroke_geometry = VertexBuffers::new();
    let mut stroke_tessellator = StrokeTessellator::new();
    stroke_tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(2.0 * LOGICAL_PIXEL_SIZE)
                .with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut stroke_geometry, |v: StrokeVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    let mut fill_geometry = VertexBuffers::new();
    let mut fill_tessellator = FillTessellator::new();
    fill_tessellator
        .tessellate_path(
            &path,
            &FillOptions::DEFAULT.with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut fill_geometry, |v: FillVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    geometry!(device, stroke_geometry, fill_geometry, "XOR gate")
}

fn build_nand_gate_geometry(device: &Device) -> (Geometry, Geometry) {
    const CIRCLE_ARC_CTRL_POS: f32 = 0.552284749831;

    let mut builder = Path::builder();
    builder.begin(point(-2.0, -2.0));
    builder.line_to(point(-2.0, 0.0));
    builder.cubic_bezier_to(
        point(-2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        point(-CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        point(0.0, 2.0),
    );
    builder.cubic_bezier_to(
        point(CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        point(2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        point(2.0, 0.0),
    );
    builder.line_to(point(2.0, -2.0));
    builder.close();
    builder.circle(point(0.0, 2.5), 0.5);
    let path = builder.build();

    let mut stroke_geometry = VertexBuffers::new();
    let mut stroke_tessellator = StrokeTessellator::new();
    stroke_tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(2.0 * LOGICAL_PIXEL_SIZE)
                .with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut stroke_geometry, |v: StrokeVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    let mut fill_geometry = VertexBuffers::new();
    let mut fill_tessellator = FillTessellator::new();
    fill_tessellator
        .tessellate_path(
            &path,
            &FillOptions::DEFAULT.with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut fill_geometry, |v: FillVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    geometry!(device, stroke_geometry, fill_geometry, "NAND gate")
}

fn build_nor_gate_geometry(device: &Device) -> (Geometry, Geometry) {
    let mut builder = Path::builder();
    builder.begin(point(-2.0, -2.35));
    builder.quadratic_bezier_to(point(0.0, -1.35), point(2.0, -2.35));
    builder.line_to(point(2.0, -1.8));
    builder.quadratic_bezier_to(point(2.0, 1.0), point(0.0, 2.0));
    builder.quadratic_bezier_to(point(-2.0, 1.0), point(-2.0, -1.8));
    builder.line_to(point(-2.0, -2.35));
    builder.close();
    builder.circle(point(0.0, 2.5), 0.5);
    let path = builder.build();

    let mut stroke_geometry = VertexBuffers::new();
    let mut stroke_tessellator = StrokeTessellator::new();
    stroke_tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(2.0 * LOGICAL_PIXEL_SIZE)
                .with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut stroke_geometry, |v: StrokeVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    let mut fill_geometry = VertexBuffers::new();
    let mut fill_tessellator = FillTessellator::new();
    fill_tessellator
        .tessellate_path(
            &path,
            &FillOptions::DEFAULT.with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut fill_geometry, |v: FillVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    geometry!(device, stroke_geometry, fill_geometry, "NOR gate")
}

fn build_xnor_gate_geometry(device: &Device) -> (Geometry, Geometry) {
    let mut builder = Path::builder();
    builder.begin(point(-2.0, -1.8));
    builder.quadratic_bezier_to(point(0.0, -0.8), point(2.0, -1.8));
    builder.quadratic_bezier_to(point(2.0, 1.0), point(0.0, 2.0));
    builder.quadratic_bezier_to(point(-2.0, 1.0), point(-2.0, -1.8));
    builder.close();
    builder.begin(point(-2.0, -2.35));
    builder.quadratic_bezier_to(point(0.0, -1.35), point(2.0, -2.35));
    builder.end(false);
    builder.circle(point(0.0, 2.5), 0.5);
    let path = builder.build();

    let mut stroke_geometry = VertexBuffers::new();
    let mut stroke_tessellator = StrokeTessellator::new();
    stroke_tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(2.0 * LOGICAL_PIXEL_SIZE)
                .with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut stroke_geometry, |v: StrokeVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    let mut fill_geometry = VertexBuffers::new();
    let mut fill_tessellator = FillTessellator::new();
    fill_tessellator
        .tessellate_path(
            &path,
            &FillOptions::DEFAULT.with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut fill_geometry, |v: FillVertex| Vertex {
                position: v.position().into(),
            }),
        )
        .expect("failed to tessellate path");

    geometry!(device, stroke_geometry, fill_geometry, "XNOR gate")
}

pub(super) struct GeometryStore {
    pub(super) and_gate_geometry: (Geometry, Geometry),
    pub(super) or_gate_geometry: (Geometry, Geometry),
    pub(super) xor_gate_geometry: (Geometry, Geometry),
    pub(super) nand_gate_geometry: (Geometry, Geometry),
    pub(super) nor_gate_geometry: (Geometry, Geometry),
    pub(super) xnor_gate_geometry: (Geometry, Geometry),
}

impl GeometryStore {
    pub(super) fn instance(device: &Device) -> &'static Self {
        use std::sync::OnceLock;

        static INSTANCE: OnceLock<GeometryStore> = OnceLock::new();
        INSTANCE.get_or_init(|| GeometryStore {
            and_gate_geometry: build_and_gate_geometry(device),
            or_gate_geometry: build_or_gate_geometry(device),
            xor_gate_geometry: build_xor_gate_geometry(device),
            nand_gate_geometry: build_nand_gate_geometry(device),
            nor_gate_geometry: build_nor_gate_geometry(device),
            xnor_gate_geometry: build_xnor_gate_geometry(device),
        })
    }
}
