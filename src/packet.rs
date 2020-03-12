/*
 * Copyright (C) 2019–2020 Miguel Rodríguez Pérez <miguel@det.uvigo.gal>
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
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Packet {
    pub dst: SocketAddrV4,
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

fn get_dst(data: &[u8]) -> Result<SocketAddrV4, String> {
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

    Ok(SocketAddrV4::new(Ipv4Addr::from(addr_bytes), port))
}

impl Packet {
    pub fn create(orig: &SocketAddrV4, data: &[u8], exit_time: Instant) -> Result<Packet, String> {
        Ok(Packet {
            dst: get_dst(data)?,
            data: orig
                .ip()
                .octets()
                .iter()
                .chain(orig.port().to_be_bytes().iter())
                .chain(&data[6..])
                .copied()
                .collect(),
            exit_time,
        })
    }

    pub fn get_duration_till_next(&self, now: Instant) -> Option<Duration> {
        Some(self.exit_time.saturating_duration_since(now))
    }

    pub fn dst(&self) -> SocketAddr {
        SocketAddr::from(self.dst)
    }
}
