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

#[macro_use]
extern crate log;

use shufflerouter::buffer::BufferPool;
use shufflerouter::packet::Packet;
use shufflerouter::queue::Queue;

use mio::net::UdpSocket;
use num_format::{SystemLocale, ToFormattedString};
use rand::distributions::{Bernoulli, Distribution, Uniform};
use signal_hook::iterator::Signals;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
/// Miguel Rodríguez Pérez <miguel@det.uvigo.gal>
/// A suffling router for Redes de Ordenadores subject
///
/// This is a simple echo server that redirects received UDP packets after a
/// random amount of time—so packets can get reordered or even dropped—.
///
///  Received packets must carry the destination address in the first four
///  bytes of the payload and the destination port as the fifth and sixth
///  byte. All of them in network byte order.
struct Opt {
    /// Listening port
    #[structopt(short = "p", long = "port", default_value = "2019")]
    port: u16,

    /// Packet drop probability
    #[structopt(short = "d", long = "drop", default_value = "0.0")]
    drop: f64,

    /// Minimum packet delay, in milliseconds
    #[structopt(short = "m", long = "min_delay", default_value = "0")]
    min_delay: u64,

    /// Packet delay randomness, in milliseconds
    #[structopt(short = "r", long = "rand_delay", default_value = "0")]
    rand_delay: u64,

    /// Verbose level
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: usize,

    /// Show log timestamp (sec, ms, ns, none)
    #[structopt(short = "t", long = "timestamp")]
    ts: Option<stderrlog::Timestamp>,
}

const SOCKACT: mio::Token = mio::Token(0);
const SIGTERM: mio::Token = mio::Token(1);

fn process_queue(
    queue: &mut Queue,
    socket: &UdpSocket,
    buffer_pool: &mut BufferPool,
) -> (usize, Duration) {
    let mut bytes_sent = 0;
    let mut extra_delay = Duration::new(0, 0);
    let now = Instant::now();

    while queue.peek().map_or(false, |p| p.exit_time <= now) {
        let p = queue.peek().unwrap();
        bytes_sent += match socket.send_to(p.data.get(), &p.dst()) {
            Ok(len) => {
                debug!("Sent {} bytes to {}", len, p.dst);
                extra_delay += now - p.exit_time;
                buffer_pool.recycle_byffer(queue.pop().unwrap().data); // Only remove transmitted packets
                len
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // We can not send more data without blocking
                break;
            }
            Err(e) => {
                warn!(
                    "Error transmitting {} bytes to {}: {}",
                    p.data.len(),
                    p.dst,
                    e
                );
                0
            }
        };
    }

    (bytes_sent, extra_delay)
}

fn main() {
    let opt = Opt::from_args();

    stderrlog::new()
        .module(module_path!())
        .verbosity(opt.verbose)
        .timestamp(opt.ts.unwrap_or(stderrlog::Timestamp::Off))
        .init()
        .unwrap();

    let drop_distribution = match Bernoulli::new(opt.drop) {
        Ok(dist) => dist,
        Err(_) => {
            error!("{} is not a valid probability value.", opt.drop);
            return;
        }
    };

    let delay_distribution = Uniform::new_inclusive(opt.min_delay, opt.min_delay + opt.rand_delay);

    let mut rng = rand::thread_rng();

    let socket = match UdpSocket::bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, opt.port))) {
        Ok(socket) => socket,
        Err(_) => {
            error!("Could not open listening socket.");
            return;
        }
    };

    let mut queue = Queue::new();

    let poll = mio::Poll::new().unwrap();
    poll.register(
        &socket,
        SOCKACT,
        mio::Ready::readable(),
        mio::PollOpt::level(),
    )
    .unwrap();

    let signals = Signals::new(&[signal_hook::SIGTERM, signal_hook::SIGINT])
        .expect("Could not capture TERM signal.");
    poll.register(
        &signals,
        SIGTERM,
        mio::Ready::readable(),
        mio::PollOpt::level(),
    )
    .unwrap();

    let mut events = mio::Events::with_capacity(32); // Just a few to store those received while transmiitting if needed
    let mut bytes_sent = 0;
    let mut extra_delay = Duration::new(0, 0);
    let mut buffer_pool = BufferPool::default();

    loop {
        let now = Instant::now();
        let max_delay = match queue.peek() {
            None => None,
            Some(packet) => match packet.get_duration_till_next(now) {
                Some(delay) if delay > extra_delay => Some(delay - extra_delay),
                Some(_delay) => Some(Duration::new(0, 0)),
                None => None,
            },
        };

        poll.reregister(
            &socket,
            SOCKACT,
            mio::Ready::readable(),
            mio::PollOpt::level(),
        )
        .unwrap();

        if let Some(packet) = queue.peek() {
            if packet.exit_time <= now {
                poll.reregister(
                    &socket,
                    SOCKACT,
                    mio::Ready::readable() | mio::Ready::writable(),
                    mio::PollOpt::level(),
                )
                .unwrap();
            }
        }

        poll.poll(&mut events, max_delay)
            .expect("Error while polling socket");

        for event in events.iter() {
            match event.token() {
                SOCKACT => {
                    if event.readiness() & mio::Ready::writable() == mio::Ready::writable() {
                        let (bytes, delay) = process_queue(&mut queue, &socket, &mut buffer_pool);
                        bytes_sent += bytes;
                        extra_delay += delay;
                    }

                    if event.readiness() & mio::Ready::readable() == mio::Ready::readable() {
                        loop {
                            // Get all pending packets
                            let mut buffer = buffer_pool.get_buffer();
                            let (len, addr) = match socket.recv_from(buffer.get_mut()) {
                                Ok((len, addr)) => match addr {
                                    SocketAddr::V4(addrv4) => (len, addrv4),
                                    _ => panic!("Unimplemented"),
                                },

                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    // We can not read more data without blocking
                                    break;
                                }
                                _ => {
                                    panic!("Error while reading datagram.");
                                }
                            };

                            debug!("Received {} bytes from {}", len, addr);

                            if drop_distribution.sample(&mut rng) {
                                info!("Τύχη decided it. Packet dropped.");
                            } else {
                                let frame_delay =
                                    Duration::from_millis(delay_distribution.sample(&mut rng));

                                info!(
                                    "Packet will be delayed for {} milliseconds",
                                    frame_delay.as_millis()
                                );

                                match Packet::create(
                                    &addr,
                                    buffer,
                                    len,
                                    Instant::now() + frame_delay,
                                ) {
                                    Ok(packet) => queue.push(packet),
                                    Err(err) => warn!("{}", err),
                                };
                            };
                        }
                    }
                }
                SIGTERM => {
                    let locale = match SystemLocale::default() {
                        Ok(locale) => locale,
                        Err(_) => SystemLocale::from_name("C").unwrap(),
                    };
                    println!(
                        "\n{} bytes sent during latest execution.",
                        bytes_sent.to_formatted_string(&locale)
                    );
                    return;
                }
                _ => unreachable!(),
            }
        }
    }
}
