# Roadmap

## M0

Local CLI, configuration, aliases, `ssh.exe` probing, `ssh -G` parsing, doctor command, and Windows ConsoleGuard skeleton.

## M1 (implemented locally; interoperability pending)

Start the official remote `mosh-server` over `ssh.exe`, parse bootstrap output, enforce timeout and
output limits, and expose a key-free `--bootstrap-only` diagnostic. Validation against a real Linux
server is still pending.

## M2 (implemented locally; interoperability pending)

Implement compatible AES-OCB encrypted datagrams, sequence validation, UDP transport, roaming
source tracking, and timing primitives using local golden vectors.

## M3 (implemented locally; full SSP semantics pending)

Port protobuf-compatible transport instructions, compressed fragments, state history, ACK handling,
retransmission, and convergence tests under duplication and reordering.

## M4 (implemented locally; terminal breadth pending)

Add a VT terminal model, renderer, terminal state application, keyboard mapping, and interactive
encrypted-session loop.

## M5-M7

Complete full upstream SSP timing/chaff behavior, real Linux interoperability, roaming recovery,
and predictive local echo.
