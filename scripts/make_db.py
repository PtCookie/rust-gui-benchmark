#!/usr/bin/env python3
"""Create the 1M-row SQLite benchmark database (same schema as bench-core::make_db)."""
import os, random, sqlite3, sys

path, rows = sys.argv[1], int(sys.argv[2])
if os.path.exists(path):
    os.remove(path)
c = sqlite3.connect(path)
c.executescript("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;"
                "CREATE TABLE events(id INTEGER PRIMARY KEY, user TEXT, kind TEXT, amount REAL, ts INTEGER, note TEXT);")
rng = random.Random(1234)
kinds = ["click", "view", "purchase", "refund", "login", "logout"]


def gen():
    for i in range(rows):
        r = rng.getrandbits(64)
        yield (i, f"user_{r % 50000:05d}", kinds[(r >> 20) % 6], (r % 1000000) / 100.0,
               1600000000 + (r >> 8) % 100000000, f"note {r % 9973} lorem ipsum dolor sit amet")


c.executemany("INSERT INTO events VALUES (?,?,?,?,?,?)", gen())
c.commit()
c.execute("CREATE INDEX idx_user ON events(user)")
c.commit()
print(f"created {path}: {rows} rows, {os.path.getsize(path) / 1e6:.0f} MB")
