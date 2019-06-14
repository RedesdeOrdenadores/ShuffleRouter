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

use std::cmp::Ordering;
use std::convert::TryInto;
use std::net;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Packet {
    pub dst: net::SocketAddr,
    pub data: Vec<u8>,
    pub exit_time: Instant,
}

impl PartialEq for Packet {
    fn eq(&self, other: &Packet) -> bool {
        self.exit_time.eq(&other.exit_time)
    }
}

impl Eq for Packet {}

impl Ord for Packet {
    fn cmp(&self, other: &Packet) -> Ordering {
        other.exit_time.cmp(&self.exit_time)
    }
}

impl PartialOrd for Packet {
    fn partial_cmp(&self, other: &Packet) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn get_dst(data: &[u8]) -> Result<net::SocketAddr, String> {
    if data.len() < 6 {
        return Err(format!(
            "Only {} bytes in received data. Minimum is six for IP + port",
            data.len()
        ));
    }

    let addr_bytes: [u8; 4] = data[..4]
        .try_into()
        .map_err(|_| "Could not extract destination address")?;
    let port = u16::from_be_bytes(
        data[4..6]
            .try_into()
            .map_err(|_| "Could not extract destination port")?,
    );

    Ok(net::SocketAddr::from((addr_bytes, port)))
}

impl Packet {
    pub fn create(data: &[u8], exit_time: Instant) -> Result<Packet, String> {
        Ok(Packet {
            dst: get_dst(data)?,
            data: Vec::from(&data[6..]),
            exit_time,
        })
    }

    pub fn get_duration_till_next(&self) -> Option<Duration> {
        let now = Instant::now();

        if now > self.exit_time {
            None
        } else {
            Some(self.exit_time.duration_since(now))
        }
    }
}
