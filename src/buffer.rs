/*
 * Copyright (C) 2021 Miguel Rodríguez Pérez <miguel@det.uvigo.gal>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

const MAX_BUFFER_SIZE: usize = 1500;
const MAX_BUFFER_CAPACITY: usize = 16 * 1024;

#[derive(Clone)]
pub struct Buffer {
    buf: [u8; MAX_BUFFER_SIZE],
    len: usize,
}

#[allow(clippy::len_without_is_empty)]
impl Buffer {
    pub fn get_mut(&mut self) -> &mut [u8; MAX_BUFFER_SIZE] {
        &mut self.buf
    }

    pub fn get(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    pub fn set_len(&mut self, len: usize) {
        self.len = len
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Default for Buffer {
    fn default() -> Buffer {
        Buffer {
            buf: [0; MAX_BUFFER_SIZE],
            len: MAX_BUFFER_SIZE,
        }
    }
}

pub struct BufferPool {
    queue: Vec<Buffer>,
}

impl BufferPool {
    pub fn get_buffer(&mut self) -> Buffer {
        if self.queue.is_empty() {
            Buffer::default()
        } else {
            self.queue.pop().unwrap()
        }
    }

    pub fn recycle_byffer(&mut self, mut buffer: Buffer) {
        buffer.set_len(MAX_BUFFER_SIZE);
        if self.queue.len() <= MAX_BUFFER_CAPACITY {
            self.queue.push(buffer)
        }
    }
}

impl Default for BufferPool {
    fn default() -> BufferPool {
        BufferPool {
            queue: Vec::with_capacity(1024),
        }
    }
}
