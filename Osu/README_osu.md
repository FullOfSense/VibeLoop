# VibeLoop — osu! Integration

Haptic feedback for osu! using [tosu](https://github.com/tosuapp/tosu) and Intiface Central.

→ See the [main VibeLoop README](../README.md) for hardware requirements, Intiface Central setup, and remote sync instructions.

---

## Additional Requirements

- osu!lazer (native Linux AppImage or Windows)
- tosu (memory reader that serves live game data over WebSocket)

---

## Install tosu

**Linux:**
```bash
# Check https://github.com/tosuapp/tosu/releases for the latest version
wget https://github.com/tosuapp/tosu/releases/download/v4.19.1/tosu-linux-v4.19.1.zip -O ~/tosu.zip
mkdir -p ~/tosu
unzip ~/tosu.zip -d ~/tosu
chmod +x ~/tosu/tosu

# Allow tosu to read game memory without sudo
sudo setcap cap_sys_ptrace=eip ~/tosu/tosu
```

**Windows:**
Download the latest release from https://github.com/tosuapp/tosu/releases and run `tosu.exe`.

---

## Install osu!lazer

**Linux:**
```bash
wget https://github.com/ppy/osu/releases/latest/download/osu.AppImage -O ~/osu.AppImage
chmod +x ~/osu.AppImage
~/osu.AppImage
```

**Windows:**
Download from https://osu.ppy.sh/home/download.

---

## Usage

Start everything in this order:

1. **Intiface Central** → Start Server → connect toy(s)
2. **tosu** — `~/tosu/tosu` (Linux) or `tosu.exe` (Windows)
3. **osu!lazer**
4. **VibeLoop** — pick a variant and run it:

```bash
# Rewarding — good hits vibrate strongly
python3 vibeloop_osu_rewarding.py

# Punishing — misses and bad hits are emphasised
python3 vibeloop_osu_punishing.py
```

**With remote sync (host mode):**
```bash
python3 vibeloop_osu_rewarding.py --relay ws://YOUR_SERVER:8765 --room ROOMCODE
```

Expected output:
```
[INFO] === VibeLoop — Osu! x Hush 2 ===
[INFO] Connected to Intiface Central!
[INFO] Using 2 device(s): Lovense Hush, Lovense Lush
[INFO] Connected to tosu v1!
[INFO] Connected to tosu v2 (spinner data)!
[INFO] Hosting room: ROOMCODE | Clients connected: 0
```

Or use the **GUI launcher** from the repo root:
```bash
python3 vibeloop_gui.py
```

---

## Haptic Mapping

### Hit Judgements

Each hit fires a short pulse. Better judgements = stronger and longer pulses. Intensity snaps up instantly on each hit, then smoothly decays back to idle.

| Judgement | Intensity | Pulse Duration |
|---|---|---|
| Perfect 300 (geki) | 50% | 0.14s |
| Great 300 | 30% | 0.11s |
| OK 100 | 15% | 0.09s |
| Meh 50 | 7% | 0.07s |
| Miss | 75% | 0.30s |

### Map Events

| Event | Effect |
|---|---|
| Map failed (collapse animation only) | 100% for 3 seconds |
| Map passed (result screen, state 2→7) | Random bursts for 6 seconds, strength scaled to accuracy |
| Escape / quit early (state 2→5) | No vibration |
| In menus / idle | Vibration off |

### Win Intensity by Accuracy

| Accuracy | Win Intensity |
|---|---|
| 100% (SS) | 100% |
| 95% | 95% |
| 80% | 80% |
| 60% | 60% |
| Below 20% | 20% (floor) |

---

## Variants

| File | Style |
|---|---|
| `vibeloop_osu_rewarding.py` | Good hits produce strong vibration. Misses still punish but positives dominate. |
| `vibeloop_osu_punishing.py` | Misses and 50s vibrate strongly. Good hits are silent. |

Both variants support `--relay`, `--room`, and `--password` for remote sync.

---

## Tuning

Edit the constants near the top of either script:

```python
# Intensity levels (0.0–1.0)
MEH     = 0.07
OK      = 0.15
GREAT   = 0.30
PERFECT = 0.50
MISS    = 0.75
FAIL    = 1.0

# Pulse durations (seconds)
MEH_DURATION     = 0.07
OK_DURATION      = 0.09
GREAT_DURATION   = 0.11
PERFECT_DURATION = 0.14
MISS_DURATION    = 0.30

# Smoothing — how fast intensity decays after a pulse (higher = faster)
SMOOTH_DOWN = 0.35

# Win pattern duration
WIN_DURATION = 6.0
```

---

## Troubleshooting

**"Cannot reach tosu"**
- Make sure tosu is running before starting VibeLoop
- Check http://localhost:24050 in your browser — it should show the tosu dashboard
- On Linux, make sure you ran `sudo setcap cap_sys_ptrace=eip ~/tosu/tosu`

**No vibration during gameplay**
- Make sure you are inside an active map, not the song select menu
- VibeLoop logs map events (pass, fail) at INFO level in the terminal

**Fail triggers on Escape**
- This is fixed — fail only triggers when `gameplay.failed` flips true in the v2 API (the collapse animation), not on escape or menu exit

**Win triggers when not playing**
- This is fixed — win only triggers on state transition `2→7` (result screen), not `2→5` (escape)

**Test tosu connection manually:**
```bash
python3 -c "
import asyncio, websockets, json
async def test():
    async with websockets.connect('ws://localhost:24050/ws') as ws:
        data = json.loads(await ws.recv())
        print('State:', data.get('menu', {}).get('state'))
        print('HP:', data.get('gameplay', {}).get('hp', {}).get('smooth'))
asyncio.run(test())
"
```
