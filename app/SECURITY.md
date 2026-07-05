# VibeLoop Security Model

## Transport

All session traffic runs over [iroh](https://iroh.computer) — QUIC with TLS,
end-to-end encrypted between host and viewer. Connections are direct
peer-to-peer when NAT hole-punching succeeds; otherwise traffic falls back to
n0's public relays, which see only encrypted bytes. There is no VibeLoop
server: nobody operates infrastructure that can read session data.

What flows through a session is a single number (vibration intensity,
0.0–1.0) about 20 times per second. No names, no messages, no game data,
no device information ever leaves the machine.

## Rooms

A room's cryptographic identity (Ed25519 key) is derived on-device with a KDF
from `lowercase(username) + password`. Consequences:

- **Password rooms are private.** A viewer with the wrong password derives a
  different key and the connection is *mathematically impossible* — there is
  no password prompt to brute-force politely, and the password itself is never
  transmitted anywhere.
- **Passwordless rooms are as public as the name.** Anyone who knows (or
  guesses) the username can join and feel the stream — which is the point for
  public streaming sessions. Be aware that they could also *host* under that
  name while you're offline (name squatting), like any system without
  accounts. If that matters, set a password.
- Room keys are deterministic: nothing is stored, nothing to lose or leak.

## Device safety

- Intensity is clamped to 0–1 at every boundary (mod output, network input,
  device command).
- Every stop path — Stop button, session end, host disconnect, mod crash,
  app exit — zeroes the bus and sends a stop-all to every device. Losing the
  host freezes nothing at high intensity: viewers zero out within seconds
  (15 s watchdog at worst, usually instantly via the connection error).
- Toy control runs through buttplug.io locally (`127.0.0.1` only); nothing
  can command your toys from outside the app.

## Reporting

Found something? Open a GitHub issue (or contact the maintainer privately for
anything sensitive).
