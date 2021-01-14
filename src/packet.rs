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
use arrayref::array_ref;
use nom::{bytes::complete::take, combinator::map};
use nom::{do_parse, named, number::complete::be_u16, IResult};
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
    Unknown(),
}

pub struct Packet {
    pub dst: SocketAddrV4,
    pub data: Buffer,
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

fn address(input: &[u8]) -> IResult<&[u8], Ipv4Addr> {
    map(take(4u8), |ip_bytes: &[u8]| {
        Ipv4Addr::from(*array_ref![ip_bytes, 0, 4])
    })(input)
}

named!(sockaddr<&[u8], (Ipv4Addr, u16)>, do_parse!(
    ip: address >>
    port: be_u16 >>
    (ip, port)
));

fn get_dst(data: &[u8]) -> Result<SocketAddrV4, PacketError> {
    let (_, (ip, port)) = sockaddr(data).map_err(|e| match e {
        nom::Err::Incomplete(len) => match len {
            nom::Needed::Unknown => PacketError::NotEnoughData(),
            nom::Needed::Size(len) => PacketError::InvalidLenth(len),
        },

        _ => PacketError::Unknown(),
    })?;

    Ok(SocketAddrV4::new(ip, port))
}

impl Packet {
    pub fn create(
        orig: &SocketAddrV4,
        mut data: Buffer,
        len: usize,
        exit_time: Instant,
    ) -> Result<Packet, PacketError> {
        let dst = get_dst(data.get())?;

        data.get_mut()[..4].copy_from_slice(&orig.ip().octets());
        data.get_mut()[4..6].copy_from_slice(&orig.port().to_be_bytes());
        data.set_len(len);

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
}
