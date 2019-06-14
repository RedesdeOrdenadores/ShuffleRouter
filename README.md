ShuffleRouter
=======

[![Build Status](https://travis-ci.org/RedesdeOrdenadores/ShuffleRouter.svg?branch=master)](https://travis-ci.org/RedesdeOrdenadores/ShuffleRouter)

A testbed for the practicals of the Redes de Ordenadores subject

## Overview

This is a simple echo server that redirects received UDP packets after a
random amount of time —so packets can get reordered or even dropped—.

Received packets **must** carry the destination address in the first four
bytes of the payload and the destination port as the fifth and sixth byte. All
of them in network byte order.

## USAGE:
    shufflerouter [FLAGS] [OPTIONS]

### FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Verbose level

### OPTIONS:
    -d, --drop <drop>              Packet drop probability [default: 0.0]
    -M, --max_delay <max_delay>    Maximum packet delay, in milliseconds [default: 0]
    -m, --min_delay <min_delay>    Minimum packet delay, in milliseconds [default: 0]
    -p, --port <port>              Listening port [default: 2019]
    -t, --timestamp <ts>           Timestamp (sec, ms, ns, none)

## Legal

Copyright ⓒ 2019 Miguel Rodríguez Pérez <miguel@det.uvigo.gal>.

This simulator is licensed under the GNU General Public License, version 3
(GPL-3.0). For information see LICENSE

