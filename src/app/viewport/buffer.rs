#![allow(dead_code)]

use crate::size_of;
use bytemuck::Pod;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use tracing_unwrap::OptionExt;
use wgpu::*;

#[inline]
const fn align_buffer_size(size: usize) -> usize {
    const ALIGN: usize = 16;
    const MASK: usize = !(ALIGN - 1);
    (size + (ALIGN - 1)) & MASK
}

struct RawBuffer {
    size: NonZeroUsize,
    buffer: Buffer,
}

impl RawBuffer {
    fn create(device: &Device, label: Option<&str>, usage: BufferUsages, min_size: usize) -> Self {
        let size = align_buffer_size(min_size);
        let size = NonZeroUsize::new(size).expect_or_log("attempted to create a zero-sized buffer");

        let buffer = device.create_buffer(&BufferDescriptor {
            label,
            size: size.get() as BufferAddress,
            usage,
            mapped_at_creation: false,
        });

        Self { size, buffer }
    }

    fn create_init(device: &Device, label: Option<&str>, usage: BufferUsages, data: &[u8]) -> Self {
        let size = align_buffer_size(data.len());
        let size = NonZeroUsize::new(size).expect_or_log("attempted to create a zero-sized buffer");

        let buffer = device.create_buffer(&BufferDescriptor {
            label,
            size: size.get() as BufferAddress,
            usage,
            mapped_at_creation: true,
        });

        let mut view = buffer.slice(..).get_mapped_range_mut();
        view.as_mut()[..data.len()].copy_from_slice(data);
        std::mem::drop(view);
        buffer.unmap();

        Self { size, buffer }
    }

    #[inline]
    fn size(&self) -> BufferSize {
        unsafe { BufferSize::new_unchecked(self.size.get() as BufferAddress) }
    }

    fn write(&self, queue: &Queue, data: &[u8]) {
        let size = align_buffer_size(data.len());
        if let Some(size) = BufferSize::new(size as BufferAddress) {
            let mut view = queue
                .write_buffer_with(&self.buffer, 0, size)
                .expect("failed to write to buffer");
            view.as_mut()[..data.len()].copy_from_slice(data);
        }
    }

    #[inline]
    fn slice(&self, len: usize) -> BufferSlice<'_> {
        let end = len as BufferAddress;
        self.buffer.slice(..end)
    }

    #[inline]
    fn as_binding(&self) -> BindingResource {
        self.buffer.as_entire_binding()
    }
}

pub struct StaticBuffer<T: Pod> {
    len: usize,
    buffer: RawBuffer,
    _t: PhantomData<*mut T>,
}

impl<T: Pod> StaticBuffer<T> {
    pub fn create(device: &Device, label: Option<&str>, usage: BufferUsages, len: usize) -> Self {
        let min_size = size_of!(T) * len;
        let buffer = RawBuffer::create(device, label, usage, min_size);

        Self {
            len,
            buffer,
            _t: PhantomData,
        }
    }

    pub fn create_init(
        device: &Device,
        label: Option<&str>,
        usage: BufferUsages,
        data: &[T],
    ) -> Self {
        let len = data.len();
        let data = bytemuck::cast_slice(data);
        let buffer = RawBuffer::create_init(device, label, usage, data);

        Self {
            len,
            buffer,
            _t: PhantomData,
        }
    }

    #[inline]
    pub fn byte_size(&self) -> BufferSize {
        self.buffer.size()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn write(&mut self, queue: &Queue, data: &[T]) {
        debug_assert!(data.len() <= self.len);
        self.buffer.write(queue, bytemuck::cast_slice(data));
    }

    #[inline]
    pub fn slice(&self) -> BufferSlice<'_> {
        let len = size_of!(T) * self.len;
        self.buffer.slice(len)
    }

    #[inline]
    pub fn as_binding(&self) -> BindingResource {
        self.buffer.as_binding()
    }
}

unsafe impl<T: Pod> Send for StaticBuffer<T> {}
unsafe impl<T: Pod> Sync for StaticBuffer<T> {}

pub struct DynamicBuffer<T: Pod> {
    label: Option<String>,
    usage: BufferUsages,
    capacity: usize,
    len: usize,
    buffer: RawBuffer,
    _t: PhantomData<*mut T>,
}

impl<T: Pod> DynamicBuffer<T> {
    pub fn create(
        device: &Device,
        label: Option<impl Into<String>>,
        usage: BufferUsages,
        capacity: usize,
    ) -> Self {
        let label = label.map(|label| label.into());
        let min_size = size_of!(T) * capacity;
        let buffer = RawBuffer::create(device, label.as_deref(), usage, min_size);

        Self {
            label,
            usage,
            capacity,
            len: 0,
            buffer,
            _t: PhantomData,
        }
    }

    pub fn create_init(
        device: &Device,
        label: Option<impl Into<String>>,
        usage: BufferUsages,
        data: &[T],
    ) -> Self {
        let label = label.map(|label| label.into());
        let len = data.len();
        let data = bytemuck::cast_slice(data);
        let buffer = RawBuffer::create_init(device, label.as_deref(), usage, data);

        Self {
            label,
            usage,
            capacity: len,
            len,
            buffer,
            _t: PhantomData,
        }
    }

    #[inline]
    pub fn byte_size(&self) -> BufferSize {
        self.buffer.size()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn write(&mut self, device: &Device, queue: &Queue, data: &[T]) {
        if data.len() > self.capacity {
            self.capacity = data.len() * 2;

            let min_size = size_of!(T) * self.capacity;
            self.buffer = RawBuffer::create(device, self.label.as_deref(), self.usage, min_size);
        }

        self.len = data.len();
        self.buffer.write(queue, bytemuck::cast_slice(data));
    }

    #[inline]
    pub fn slice(&self) -> BufferSlice<'_> {
        let len = size_of!(T) * self.len;
        self.buffer.slice(len)
    }
}

unsafe impl<T: Pod> Send for DynamicBuffer<T> {}
unsafe impl<T: Pod> Sync for DynamicBuffer<T> {}
