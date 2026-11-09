"""Incremental, newline-delimited JSON observations for real-window checks.

Readers subscribe to exact prefixes. Latest-only polling retains no event history;
callers which count, filter or slice past events opt into history explicitly.
"""
import json
import os
from pathlib import Path


class LogReader:
    def __init__(self, path, prefixes, history=False):
        self.path = Path(path)
        self.prefixes = tuple(prefixes)
        self.keep_history = history
        self.bytes_read = 0
        self.reset()

    def reset(self):
        """Start a new app session, including truncate-and-regrow of the same inode."""
        self.offset, self.partial, self.identity = 0, b'', None
        self.values, self.current = {}, None
        self.history = [] if self.keep_history else None

    def poll(self):
        """Read new bytes and return only newly completed subscribed records, in order."""
        with self.path.open('rb') as stream:
            stat = os.fstat(stream.fileno())
            identity = (stat.st_dev, stat.st_ino)
            if identity != self.identity or stat.st_size < self.offset:
                self.reset()
            self.identity = identity
            stream.seek(self.offset)
            data = stream.read()
            self.offset = stream.tell()
        self.bytes_read += len(data)
        if not data:
            return []
        chunks = (self.partial + data).split(b'\n')
        self.partial = chunks.pop()
        records = []
        for chunk in chunks:
            # Decode only complete lines, even when a write splits a UTF-8 codepoint.
            line = chunk.decode('utf-8', errors='replace')
            for prefix in self.prefixes:
                if line.startswith(prefix):
                    value = json.loads(line[len(prefix):])
                    self.values[prefix] = self.current = value
                    records.append((prefix, value))
                    break
        if self.history is not None:
            self.history.extend(value for _, value in records)
        return records

    def latest(self, prefix=None):
        self.poll()
        return self.current if prefix is None else self.values.get(prefix)

    def entries(self):
        assert self.history is not None, 'event history must be requested at construction'
        self.poll()
        return self.history


_READERS = {}


def reader(path, prefixes, history=False):
    """Reuse a subscription across polls; each subscription reads each byte only once."""
    if isinstance(prefixes, str):
        prefixes = (prefixes,)
    key = (Path(path), tuple(prefixes), history)
    if key not in _READERS:
        _READERS[key] = LogReader(path, prefixes, history)
    return _READERS[key]


def log_latest(path, prefixes='AUDIT '):
    return reader(path, prefixes).latest()


def log_entries(path, prefixes='AUDIT '):
    return reader(path, prefixes, history=True).entries()


def reset_log(path):
    """Call before launching an app into a reused log path."""
    for (name, _, _), subscription in _READERS.items():
        if name == Path(path):
            subscription.reset()


def log_tail(path, size=4000):
    """Bounded diagnostics on failure, without rereading a multi-megabyte log."""
    with Path(path).open('rb') as stream:
        stream.seek(0, 2)
        stream.seek(max(0, stream.tell() - size))
        return stream.read().decode('utf-8', errors='replace')
