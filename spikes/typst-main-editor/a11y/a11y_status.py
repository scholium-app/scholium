"""Minimal org.a11y.Bus/org.a11y.Status for an isolated session bus (test harness only)."""
import sys
from gi.repository import Gio, GLib

XML = """<node>
 <interface name="org.a11y.Bus"><method name="GetAddress"><arg type="s" direction="out"/></method></interface>
 <interface name="org.a11y.Status">
  <property name="IsEnabled" type="b" access="readwrite"/>
  <property name="ScreenReaderEnabled" type="b" access="readwrite"/>
 </interface></node>"""
address = sys.argv[1]
props = {"IsEnabled": True, "ScreenReaderEnabled": False}

def call(conn, sender, path, iface, method, params, invocation):
    invocation.return_value(GLib.Variant("(s)", (address,)))

def get(conn, sender, path, iface, name):
    return GLib.Variant("b", props[name])

def set_(conn, sender, path, iface, name, value):
    props[name] = value.unpack()
    conn.emit_signal(None, path, "org.freedesktop.DBus.Properties", "PropertiesChanged",
                     GLib.Variant("(sa{sv}as)", (iface, {name: value}, [])))
    return True

def acquired(conn, name):
    info = Gio.DBusNodeInfo.new_for_xml(XML)
    for iface in info.interfaces:
        conn.register_object("/org/a11y/bus", iface, call, get, set_)

Gio.bus_own_name(Gio.BusType.SESSION, "org.a11y.Bus", Gio.BusNameOwnerFlags.NONE, acquired, None, None)
print("ready", flush=True)
GLib.MainLoop().run()
