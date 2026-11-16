"""GTK 4 window with one menu button that fills it. The popover of the button hides on presses
elsewhere. The script prints "shown" and "closed" for each popover change, and "key" for each key
press that reaches the popover."""

import sys

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import Gtk  # noqa: E402


def say(word):
    print(word, flush=True)


def activate(app):
    window = Gtk.ApplicationWindow(application=app, title="popover")
    window.set_default_size(400, 300)
    popover = Gtk.Popover()
    popover.set_child(Gtk.Label(label="popover content"))
    popover.set_size_request(160, 80)
    popover.connect("show", lambda *_: say("shown"))
    popover.connect("closed", lambda *_: say("closed"))
    keys = Gtk.EventControllerKey()
    keys.connect("key-pressed", lambda *_: say("key") or False)
    popover.add_controller(keys)
    button = Gtk.MenuButton(label="open", popover=popover)
    button.set_hexpand(True)
    button.set_vexpand(True)
    window.set_child(button)
    window.present()
    say("ready")


app = Gtk.Application(application_id="org.example.popover")
app.connect("activate", activate)
sys.exit(app.run([]))
