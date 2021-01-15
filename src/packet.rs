/*
 * Copyright (C) 2019–2021 Miguel Rodríguez Pérez <miguel@det.uvigo.gal>
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

use super::buffer::Buffer;
use nom::{
    combinator::map,
    number::streaming::{be_u16, be_u8},
    sequence::tuple,
    IResult,
};
use std::cmp::Ordering;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("need {0} bytes of data. Minimum is six for IP + port")]
    InvalidLenth(core::num::NonZeroUsize),
    #[error("not enough data. Minimum is six for IP + port")]
    NotEnoughData(),
    #[error("sorry, could not decode the packet header")]
    Unknown,
}

impl<E> From<nom::Err<E>> for PacketError {
    fn from(error: nom::Err<E>) -> Self {
        match error {
            nom::Err::Incomplete(len) => match len {
                nom::Needed::Unknown => PacketError::NotEnoughData(),
                nom::Needed::Size(len) => PacketError::InvalidLenth(len),
            },
            _ => PacketError::Unknown,
        }
    }
}

pub struct Packet {
    dst: SocketAddrV4,
    data: Buffer,
    exit_time: Instant,
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

fn address(input: &[u8]) -> IResult<&[u8], Ipv4Addr> {
    map(tuple((be_u8, be_u8, be_u8, be_u8)), |(a, b, c, d)| {
        Ipv4Addr::new(a, b, c, d)
    })(input)
}

fn sockaddr(input: &[u8]) -> IResult<&[u8], SocketAddrV4> {
    map(tuple((address, be_u16)), |(ip, port)| {
        SocketAddrV4::new(ip, port)
    })(input)
}

fn get_dst(data: &[u8]) -> Result<SocketAddrV4, PacketError> {
    Ok(sockaddr(data).map(|(_, addr)| addr)?)
}

impl Packet {
    pub fn create(
        orig: &SocketAddrV4,
        mut data: Buffer,
        exit_time: Instant,
    ) -> Result<Packet, PacketError> {
        let dst = get_dst(&data)?;

        data[..4].copy_from_slice(&orig.ip().octets());
        data[4..6].copy_from_slice(&orig.port().to_be_bytes());

        Ok(Packet {
            dst,
            data,
            exit_time,
        })
    }

    pub fn get_duration_till_next(&self, now: Instant) -> Option<Duration> {
        Some(self.exit_time.saturating_duration_since(now))
    }

    pub fn dst(&self) -> SocketAddr {
        SocketAddr::from(self.dst)
    }

    pub fn get(&self) -> &Buffer {
        &self.data
    }

    pub fn exit_time(&self) -> Instant {
        self.exit_time
    }
}

impl From<Packet> for Buffer {
    fn from(packet: Packet) -> Buffer {
        packet.data
    }
}
