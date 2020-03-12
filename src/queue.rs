/*
 * Copyright (C) 2019 Miguel Rodríguez Pérez <miguel@det.uvigo.gal>
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

use crate::packet::Packet;

use std::collections::binary_heap;

#[derive(Default)]
pub struct Queue {
    queue: binary_heap::BinaryHeap<Packet>,
}

impl Queue {
    pub fn new() -> Queue {
        Queue {
            queue: binary_heap::BinaryHeap::new(),
        }
    }

    pub fn peek(&self) -> Option<&Packet> {
        self.queue.peek()
    }

    pub fn pop(&mut self) -> Option<Packet> {
        self.queue.pop()
    }

    pub fn push(&mut self, packet: Packet) {
        self.queue.push(packet)
    }
}
