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

#[macro_use]
extern crate log;

mod packet;
mod queue;

use packet::Packet;
use queue::Queue;

use mio::net::UdpSocket;
use rand::distributions::{Bernoulli, Distribution, Uniform};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
/// Miguel Rodríguez Pérez <miguel@det.uvigo.gal>
/// A suffling router for Redes de Ordenadores subject
///
/// This is a simple echo server that redirects received UDP packets after a
/// random amount of time —so packets can get reordered or even dropped—.
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

const RECEIVER: mio::Token = mio::Token(0);

fn process_queue(queue: &mut Queue, socket: &UdpSocket) {
    while queue
        .peek()
        .map_or(false, |p| p.exit_time <= Instant::now())
    {
        let p = queue.pop().unwrap();
        match socket.send_to(&p.data, &p.dst) {
            Ok(len) => info!("Sent {} bytes to {}", len, p.dst),
            Err(e) => warn!(
                "Error transmitting {} bytes to {}: {}",
                p.data.len(),
                p.dst,
                e
            ),
        }
    }
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

    let mut buffer = [0; u16::max_value() as usize];

    let poll = mio::Poll::new().unwrap();
    poll.register(
        &socket,
        RECEIVER,
        mio::Ready::readable(),
        mio::PollOpt::edge(),
    )
    .unwrap();

    let mut events = mio::Events::with_capacity(32); // Just a few to store those received while transmiitting if needed

    loop {
        process_queue(&mut queue, &socket);

        let max_delay = match queue.peek() {
            None => None,
            Some(packet) => packet.get_duration_till_next(),
        };

        poll.poll(&mut events, max_delay)
            .expect("Error while polling socket");

        for event in events.iter() {
            match event.token() {
                RECEIVER => {
                    let (len, addr) = socket
                        .recv_from(&mut buffer)
                        .expect("Error while reading datagram.");

                    info!("Received {} bytes from {}", len, addr);

                    if drop_distribution.sample(&mut rng) {
                        info!("Fortuna made me do it. Packet dropped.");
                    } else {
                        let frame_delay =
                            Duration::from_millis(delay_distribution.sample(&mut rng));

                        info!(
                            "Packet will be delayed for {} milliseconds",
                            frame_delay.as_millis()
                        );

                        match Packet::create(&buffer[..len], Instant::now() + frame_delay) {
                            Ok(packet) => queue.push(packet),
                            Err(err) => warn!("{}", err),
                        };
                    };
                }
                _ => unreachable!(),
            }
        }
    }
}
