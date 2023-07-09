use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};
use std::ops::*;

macro_rules! def_vec2 {
    ($(#[$attr:meta])* $name:ident[$e:ty]) => {
        $(#[$attr])*
        #[derive(Debug, Default, Clone, Copy, Zeroable, Pod, Serialize, Deserialize)]
        #[repr(C)]
        pub struct $name {
            pub x: $e,
            pub y: $e,
        }

        impl Add for $name {
            type Output = Self;

            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self {
                    x: self.x + rhs.x,
                    y: self.y + rhs.y,
                }
            }
        }

        impl Sub for $name {
            type Output = Self;

            #[inline]
            fn sub(self, rhs: Self) -> Self::Output {
                Self {
                    x: self.x - rhs.x,
                    y: self.y - rhs.y,
                }
            }
        }

        impl Mul for $name {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: Self) -> Self::Output {
                Self {
                    x: self.x * rhs.x,
                    y: self.y * rhs.y,
                }
            }
        }

        impl Div for $name {
            type Output = Self;

            #[inline]
            fn div(self, rhs: Self) -> Self::Output {
                Self {
                    x: self.x / rhs.x,
                    y: self.y / rhs.y,
                }
            }
        }

        impl Rem for $name {
            type Output = Self;

            #[inline]
            fn rem(self, rhs: Self) -> Self::Output {
                Self {
                    x: self.x % rhs.x,
                    y: self.y % rhs.y,
                }
            }
        }

        impl Neg for $name {
            type Output = Self;

            #[inline]
            fn neg(self) -> Self::Output {
                Self {
                    x: -self.x,
                    y: -self.y,
                }
            }
        }

        impl AddAssign for $name {
            #[inline]
            fn add_assign(&mut self, rhs: Self) {
                self.x += rhs.x;
                self.y += rhs.y;
            }
        }

        impl SubAssign for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: Self) {
                self.x -= rhs.x;
                self.y -= rhs.y;
            }
        }

        impl MulAssign for $name {
            #[inline]
            fn mul_assign(&mut self, rhs: Self) {
                self.x *= rhs.x;
                self.y *= rhs.y;
            }
        }

        impl DivAssign for $name {
            #[inline]
            fn div_assign(&mut self, rhs: Self) {
                self.x /= rhs.x;
                self.y /= rhs.y;
            }
        }

        impl RemAssign for $name {
            #[inline]
            fn rem_assign(&mut self, rhs: Self) {
                self.x %= rhs.x;
                self.y %= rhs.y;
            }
        }

        impl Add<$e> for $name {
            type Output = Self;

            #[inline]
            fn add(self, rhs: $e) -> Self::Output {
                Self {
                    x: self.x + rhs,
                    y: self.y + rhs,
                }
            }
        }

        impl Sub<$e> for $name {
            type Output = Self;

            #[inline]
            fn sub(self, rhs: $e) -> Self::Output {
                Self {
                    x: self.x - rhs,
                    y: self.y - rhs,
                }
            }
        }

        impl Mul<$e> for $name {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: $e) -> Self::Output {
                Self {
                    x: self.x * rhs,
                    y: self.y * rhs,
                }
            }
        }

        impl Div<$e> for $name {
            type Output = Self;

            #[inline]
            fn div(self, rhs: $e) -> Self::Output {
                Self {
                    x: self.x / rhs,
                    y: self.y / rhs,
                }
            }
        }

        impl Rem<$e> for $name {
            type Output = Self;

            #[inline]
            fn rem(self, rhs: $e) -> Self::Output {
                Self {
                    x: self.x % rhs,
                    y: self.y % rhs,
                }
            }
        }

        impl AddAssign<$e> for $name {
            #[inline]
            fn add_assign(&mut self, rhs: $e) {
                self.x += rhs;
                self.y += rhs;
            }
        }

        impl SubAssign<$e> for $name {
            #[inline]
            fn sub_assign(&mut self, rhs: $e) {
                self.x -= rhs;
                self.y -= rhs;
            }
        }

        impl MulAssign<$e> for $name {
            #[inline]
            fn mul_assign(&mut self, rhs: $e) {
                self.x *= rhs;
                self.y *= rhs;
            }
        }

        impl DivAssign<$e> for $name {
            #[inline]
            fn div_assign(&mut self, rhs: $e) {
                self.x /= rhs;
                self.y /= rhs;
            }
        }

        impl RemAssign<$e> for $name {
            #[inline]
            fn rem_assign(&mut self, rhs: $e) {
                self.x %= rhs;
                self.y %= rhs;
            }
        }

        #[allow(dead_code)]
        impl $name {
            pub const ZERO: Self = Self {
                x: 0 as $e,
                y: 0 as $e,
            };

            pub const MIN: Self = Self {
                x: <$e>::MIN,
                y: <$e>::MIN,
            };

            pub const MAX: Self = Self {
                x: <$e>::MAX,
                y: <$e>::MAX,
            };

            #[inline]
            pub const fn new(x: $e, y: $e) -> Self {
                Self { x, y }
            }

            #[inline]
            pub const fn to_array(self) -> [$e; 2] {
                [self.x, self.y]
            }

            #[inline]
            pub fn dot(self, rhs: Self) -> $e {
                let prod = self * rhs;
                prod.x + prod.y
            }

            #[inline]
            pub fn cross(self, rhs: Self) -> $e {
                (self.x * rhs.y) - (self.y * rhs.x)
            }

            #[inline]
            pub fn abs(self) -> Self {
                Self {
                    x: self.x.abs(),
                    y: self.y.abs(),
                }
            }

            #[inline]
            pub fn min(self, rhs: Self) -> Self {
                Self {
                    x: self.x.min(rhs.x),
                    y: self.y.min(rhs.y),
                }
            }

            #[inline]
            pub fn max(self, rhs: Self) -> Self {
                Self {
                    x: self.x.max(rhs.x),
                    y: self.y.max(rhs.y),
                }
            }
        }
    };
}

def_vec2!(
    #[derive(PartialEq, Eq, Hash)]
    Vec2i[i32]
);

impl Vec2i {
    #[inline]
    pub fn to_vec2f(self) -> Vec2f {
        Vec2f {
            x: self.x as f32,
            y: self.y as f32,
        }
    }
}

def_vec2!(
    #[derive(PartialEq)]
    Vec2f[f32]
);

#[allow(dead_code)]
impl Vec2f {
    #[inline]
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }

    #[inline]
    pub fn normalized(self) -> Self {
        self / self.len()
    }

    #[inline]
    pub fn round(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
        }
    }

    #[inline]
    pub fn floor(self) -> Self {
        Self {
            x: self.x.floor(),
            y: self.y.floor(),
        }
    }

    #[inline]
    pub fn ceil(self) -> Self {
        Self {
            x: self.x.ceil(),
            y: self.y.ceil(),
        }
    }

    #[inline]
    pub fn to_vec2i(self) -> Vec2i {
        Vec2i {
            x: self.x as i32,
            y: self.y as i32,
        }
    }
}

impl From<egui::Vec2> for Vec2f {
    #[inline]
    fn from(value: egui::Vec2) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Rectangle {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

#[allow(dead_code)]
impl Rectangle {
    pub fn contains(&self, p: Vec2f) -> bool {
        (p.x >= self.left) && (p.x <= self.right) && (p.y >= self.bottom) && (p.y <= self.top)
    }

    pub fn center(&self) -> Vec2f {
        let min = Vec2f::new(self.left, self.bottom);
        let max = Vec2f::new(self.right, self.top);
        (min + max) * 0.5
    }

    #[inline]
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    #[inline]
    pub fn height(&self) -> f32 {
        self.top - self.bottom
    }
}

pub struct Triangle {
    pub a: Vec2f,
    pub b: Vec2f,
    pub c: Vec2f,
}

impl Triangle {
    pub fn contains(&self, p: Vec2f) -> bool {
        let ca = self.a - self.c;
        let ab = self.b - self.a;
        let cp = p - self.c;
        let ap = p - self.a;
        let s = ca.cross(cp);
        let t = ab.cross(ap);

        if ((s < 0.0) != (t < 0.0)) && (s != 0.0) && (t != 0.0) {
            return false;
        }

        let bc = self.c - self.b;
        let bp = p - self.c;
        let d = bc.cross(bp);

        (d == 0.0) || ((d < 0.0) == (s + t <= 0.0))
    }
}
