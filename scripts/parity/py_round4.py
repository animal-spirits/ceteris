"""The fixture of `py_round_and_repr_match_python` (crates/ceteris-pack/tests/py_round.rs).

    python3 -I scripts/parity/py_round4.py > crates/ceteris-pack/tests/data/py_round4.txt

One line per value: its IEEE bits in hex, `repr(round(x, 4))` and `repr(x)`. The values cover
coordinates, exact ties at the fifth decimal (odd multiples of 1/32 and of other powers of two),
near ties, every decimal magnitude and random bit patterns.
"""

import random, struct, sys
rng = random.Random(20001)
xs = [0.0, -0.0, 1.0, -1.0, 0.5, 1e-4, -1e-4, 5e-5, -5e-5, 4.9999e-5, 1e-5, 1e16, 1e15, 123456789012345678.0,
      1e-300, 5e-324, 1.7976931348623157e308, 180.0, -180.0, 90.0, 85.0511287798066, 0.1, 0.2, 0.3]
for _ in range(500):
    xs.append(rng.uniform(-180, 180))
for _ in range(200):
    m = rng.randrange(-2**20, 2**20) | 1
    xs.append(m / 32)
    xs.append(m / 2**rng.randrange(5, 12))
for _ in range(200):
    xs.append((rng.randrange(-1800000, 1800000) + 0.5) * 1e-4)
for _ in range(200):
    xs.append(rng.uniform(-1, 1) * 10.0 ** rng.randrange(-320, 309))
for _ in range(100):
    xs.append(struct.unpack("<d", struct.pack("<Q", rng.getrandbits(63)))[0] * rng.choice([1, -1]))
for x in xs:
    if x != x or x in (float("inf"), float("-inf")):
        continue
    print(f"{struct.unpack('<Q', struct.pack('<d', x))[0]:016x} {round(x, 4)!r} {x!r}")
