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

#[macro_use]
extern crate log;

use shufflerouter::buffer::BufferPool;
use shufflerouter::packet::Packet;
use shufflerouter::queue::Queue;

use anyhow::Result;
use clap::{crate_authors, crate_version, Clap};
use mio::{Interest, Token};
use rand::distributions::{Bernoulli, Distribution, Uniform};
use std::{
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use std::{
    thread,
    time::{Duration, Instant},
};
use tokio::signal;

/// A shuffling router for Redes de Ordenadores subject
///
/// This is a simple echo server that redirects received UDP packets after a
/// random amount of time—so packets can get reordered or even dropped—.
///
///  Received packets must carry the destination address in the first four
///  bytes of the payload and the destination port as the fifth and sixth
///  byte. All of them in network byte order.
#[derive(Clap, Debug)]
#[clap(version = crate_version!(), author = crate_authors!())]
struct Opt {
    /// Listening port
    #[clap(short = 'p', long = "port", default_value = "2021")]
    port: u16,

    /// Packet drop probability
    #[clap(short = 'd', long = "drop", default_value = "0.0")]
    drop: f64,

    /// Minimum packet delay, in milliseconds
    #[clap(short = 'm', long = "min_delay", default_value = "0")]
    min_delay: u64,

    /// Packet delay randomness, in milliseconds
    #[clap(short = 'r', long = "rand_delay", default_value = "0")]
    rand_delay: u64,

    /// Verbose level
    #[clap(short = 'v', long = "verbose", parse(from_occurrences))]
    verbose: usize,

    /// Show log timestamp (sec, ms, ns, none)
    #[clap(short = 't', long = "timestamp")]
    ts: Option<stderrlog::Timestamp>,

    /// EXPERIMENTAL: Multithreaded version
    #[clap(short = 'j', long = "parallel")]
    parallel: bool,
}

fn process_queue(
    queue: &mut Queue,
    socket: &mio::net::UdpSocket,
    buffer_pool: &mut BufferPool,
) -> usize {
    let mut bytes_sent = 0;
    let now = Instant::now();

    while queue.peek().map_or(false, |p| p.exit_time() <= now) {
        let p = queue.peek().unwrap();
        bytes_sent += match socket.send_to(p.get(), p.dst()) {
            Ok(len) => {
                debug!("Sent {} bytes to {}", len, p.dst());
                buffer_pool.recycle_buffer(queue.pop().unwrap().into()); // Only remove transmitted packets
                len
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // We can not send more data without blocking
                break;
            }
            Err(e) => {
                warn!(
                    "Error transmitting {} bytes to {}: {}",
                    p.get().len(),
                    p.dst(),
                    e
                );
                buffer_pool.recycle_buffer(queue.pop().unwrap().into()); // Remove the packet causing the error
                0
            }
        };
    }

    bytes_sent
}

fn process_traffic(
    socket: UdpSocket,
    drop_distribution: impl Distribution<bool>,
    delay_distribution: impl Distribution<u64>,
    bytes_sent: Arc<AtomicUsize>,
) -> Result<()> {
    let mut rng = rand::thread_rng();
    let mut queue = Queue::new();
    let mut poll = mio::Poll::new()?;

    let mut socket = mio::net::UdpSocket::from_std(socket);

    const SOCKACT: Token = Token(0);

    poll.registry()
        .register(&mut socket, SOCKACT, Interest::READABLE)?;

    let mut events = mio::Events::with_capacity(32); // Just a few to store those received while transmiitting if needed
    let mut buffer_pool = BufferPool::default();

    loop {
        let now = Instant::now();
        let max_delay = match queue.peek() {
            None => None,
            Some(packet) => packet.get_duration_till_next(now),
        };

        poll.registry().reregister(
            &mut socket,
            SOCKACT,
            match queue.peek() {
                Some(packet) if packet.exit_time() <= now => {
                    Interest::READABLE | Interest::WRITABLE
                }
                _ => Interest::READABLE,
            },
        )?;

        poll.poll(&mut events, max_delay)?;

        for event in &events {
            match event.token() {
                SOCKACT => {
                    if event.is_writable() {
                        bytes_sent.fetch_add(
                            process_queue(&mut queue, &socket, &mut buffer_pool),
                            Ordering::Relaxed,
                        );
                    }

                    if event.is_readable() {
                        loop {
                            // Get all pending packets
                            let mut buffer = buffer_pool.get_buffer();
                            let (len, addr) = match socket.recv_from(&mut buffer) {
                                Ok((len, addr)) => match addr {
                                    SocketAddr::V4(addrv4) => (len, addrv4),
                                    _ => panic!("Unimplemented"),
                                },

                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    // We can not read more data without blocking
                                    break;
                                }
                                _ => {
                                    panic!("Error while reading datagram.");
                                }
                            };
                            let arrival_time = Instant::now();
                            buffer.set_len(len);

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

                                if let Err(e) =
                                    Packet::create(addr, buffer, arrival_time + frame_delay)
                                        .map(|packet| queue.push(packet))
                                {
                                    warn!("Could not parse packet {:?}", e);
                                }
                            };
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let opt = Opt::parse();

    stderrlog::new()
        .module(module_path!())
        .verbosity(opt.verbose)
        .timestamp(opt.ts.unwrap_or(stderrlog::Timestamp::Off))
        .init()?;

    let drop_distribution = Bernoulli::new(opt.drop)?;

    let delay_distribution = Uniform::new_inclusive(opt.min_delay, opt.min_delay + opt.rand_delay);

    let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, opt.port)))?;
    socket.set_nonblocking(true)?;

    let bytes_sent = Arc::new(AtomicUsize::default());

    for _i in 1..=if opt.parallel { num_cpus::get() } else { 1 } {
        let bytes_sent = bytes_sent.clone();
        let socket = socket.try_clone()?;

        let _thread = thread::spawn(move || {
            if let Err(e) =
                process_traffic(socket, drop_distribution, delay_distribution, bytes_sent)
            {
                warn!("Error while processing traffic: {:?}", e);
            };
        });
    }

    signal::ctrl_c().await?;

    println!(
        "\n{} bytes sent during latest execution.",
        bytes_sent.fetch_add(0, Ordering::Relaxed) // Trick to cast to usize
    );

    Ok(())
}
