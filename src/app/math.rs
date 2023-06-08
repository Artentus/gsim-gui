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
            #[inline]
            pub const fn new(x: $e, y: $e) -> Self {
                Self { x, y }
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

def_vec2!(Vec2f[f32]);

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

impl From<lyon::math::Point> for Vec2f {
    #[inline]
    fn from(value: lyon::math::Point) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Clone, Copy)]
pub struct BoundingBox {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl BoundingBox {
    pub fn contains(&self, p: Vec2f) -> bool {
        (p.x >= self.left) && (p.x <= self.right) && (p.y >= self.bottom) && (p.y <= self.top)
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
