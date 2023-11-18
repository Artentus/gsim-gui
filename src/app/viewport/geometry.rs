use vello::kurbo::*;

const CIRCLE_ARC_CTRL_POS: f64 = 0.55228474983079;

trait BezPathExt {
    fn circle(&mut self, center: impl Into<Point>, radius: f64);
}

impl BezPathExt for BezPath {
    fn circle(&mut self, center: impl Into<Point>, radius: f64) {
        let center = center.into();

        #[inline]
        fn point_mul((x, y): (f64, f64), s: f64) -> (f64, f64) {
            (x * s, y * s)
        }

        self.move_to(center + point_mul((0.0, -1.0), radius));
        self.curve_to(
            center + point_mul((-CIRCLE_ARC_CTRL_POS, -1.0), radius),
            center + point_mul((-1.0, -CIRCLE_ARC_CTRL_POS), radius),
            center + point_mul((-1.0, 0.0), radius),
        );
        self.curve_to(
            center + point_mul((-1.0, CIRCLE_ARC_CTRL_POS), radius),
            center + point_mul((-CIRCLE_ARC_CTRL_POS, 1.0), radius),
            center + point_mul((0.0, 1.0), radius),
        );
        self.curve_to(
            center + point_mul((CIRCLE_ARC_CTRL_POS, 1.0), radius),
            center + point_mul((1.0, CIRCLE_ARC_CTRL_POS), radius),
            center + point_mul((1.0, 0.0), radius),
        );
        self.curve_to(
            center + point_mul((1.0, -CIRCLE_ARC_CTRL_POS), radius),
            center + point_mul((CIRCLE_ARC_CTRL_POS, -1.0), radius),
            center + point_mul((0.0, -1.0), radius),
        );
        self.close_path();
    }
}

pub(super) enum Geometry {
    Same(BezPath),
    Different(BezPath, BezPath),
}

impl Geometry {
    pub(super) fn fill_path(&self) -> &BezPath {
        match self {
            Geometry::Same(path) => path,
            Geometry::Different(path, _) => path,
        }
    }

    pub(super) fn stroke_path(&self) -> &BezPath {
        match self {
            Geometry::Same(path) => path,
            Geometry::Different(_, path) => path,
        }
    }
}

fn build_input_geometry() -> Geometry {
    let mut path = BezPath::new();
    path.move_to((-1.0, -1.0));
    path.line_to((-1.0, 1.0));
    path.line_to((1.0, 1.0));
    path.line_to((1.0, -1.0));
    path.close_path();

    Geometry::Same(path)
}

fn build_output_geometry() -> Geometry {
    let mut path = BezPath::new();
    path.circle((0.0, 0.0), 1.0);

    Geometry::Same(path)
}

fn build_and_gate_geometry() -> Geometry {
    let mut path = BezPath::new();
    path.move_to((-2.0, -2.0));
    path.line_to((-2.0, 0.0));
    path.curve_to(
        (-2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        (-CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        (0.0, 2.0),
    );
    path.curve_to(
        (CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        (2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        (2.0, 0.0),
    );
    path.line_to((2.0, -2.0));
    path.close_path();

    Geometry::Same(path)
}

fn build_or_gate_geometry() -> Geometry {
    let mut path = BezPath::new();
    path.move_to((-2.0, -2.35));
    path.quad_to((0.0, -1.35), (2.0, -2.35));
    path.line_to((2.0, -1.8));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.quad_to((-2.0, 1.0), (-2.0, -1.8));
    path.line_to((-2.0, -2.35));
    path.close_path();

    Geometry::Same(path)
}

fn build_xor_gate_geometry() -> Geometry {
    let mut fill_path = BezPath::new();
    fill_path.move_to((-2.0, -1.8));
    fill_path.quad_to((0.0, -0.8), (2.0, -1.8));
    fill_path.quad_to((2.0, 1.0), (0.0, 2.0));
    fill_path.quad_to((-2.0, 1.0), (-2.0, -1.8));
    fill_path.close_path();

    let mut stroke_path = fill_path.clone();
    stroke_path.move_to((-2.0, -2.35));
    stroke_path.quad_to((0.0, -1.35), (2.0, -2.35));

    Geometry::Different(fill_path, stroke_path)
}

fn build_nand_gate_geometry() -> Geometry {
    let mut path = BezPath::new();
    path.circle((0.0, 2.5), 0.5);
    path.move_to((-2.0, -2.0));
    path.line_to((-2.0, 0.0));
    path.curve_to(
        (-2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        (-CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        (0.0, 2.0),
    );
    path.curve_to(
        (CIRCLE_ARC_CTRL_POS * 2.0, 2.0),
        (2.0, CIRCLE_ARC_CTRL_POS * 2.0),
        (2.0, 0.0),
    );
    path.line_to((2.0, -2.0));
    path.close_path();

    Geometry::Same(path)
}

fn build_nor_gate_geometry() -> Geometry {
    let mut path = BezPath::new();
    path.circle((0.0, 2.5), 0.5);
    path.move_to((-2.0, -2.35));
    path.quad_to((0.0, -1.35), (2.0, -2.35));
    path.line_to((2.0, -1.8));
    path.quad_to((2.0, 1.0), (0.0, 2.0));
    path.quad_to((-2.0, 1.0), (-2.0, -1.8));
    path.line_to((-2.0, -2.35));
    path.close_path();

    Geometry::Same(path)
}

fn build_xnor_gate_geometry() -> Geometry {
    let mut fill_path = BezPath::new();
    fill_path.circle((0.0, 2.5), 0.5);
    fill_path.move_to((-2.0, -1.8));
    fill_path.quad_to((0.0, -0.8), (2.0, -1.8));
    fill_path.quad_to((2.0, 1.0), (0.0, 2.0));
    fill_path.quad_to((-2.0, 1.0), (-2.0, -1.8));
    fill_path.close_path();

    let mut stroke_path = fill_path.clone();
    stroke_path.move_to((-2.0, -2.35));
    stroke_path.quad_to((0.0, -1.35), (2.0, -2.35));

    Geometry::Different(fill_path, stroke_path)
}

pub(super) struct GeometryStore {
    pub(super) input_geometry: Geometry,
    pub(super) output_geometry: Geometry,
    pub(super) and_gate_geometry: Geometry,
    pub(super) or_gate_geometry: Geometry,
    pub(super) xor_gate_geometry: Geometry,
    pub(super) nand_gate_geometry: Geometry,
    pub(super) nor_gate_geometry: Geometry,
    pub(super) xnor_gate_geometry: Geometry,
}

impl GeometryStore {
    pub(super) fn new() -> Self {
        Self {
            input_geometry: build_input_geometry(),
            output_geometry: build_output_geometry(),
            and_gate_geometry: build_and_gate_geometry(),
            or_gate_geometry: build_or_gate_geometry(),
            xor_gate_geometry: build_xor_gate_geometry(),
            nand_gate_geometry: build_nand_gate_geometry(),
            nor_gate_geometry: build_nor_gate_geometry(),
            xnor_gate_geometry: build_xnor_gate_geometry(),
        }
    }
}
