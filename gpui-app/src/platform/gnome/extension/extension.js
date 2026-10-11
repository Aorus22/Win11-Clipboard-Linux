// Clipboard History (GPUI) helper — GNOME Shell extension.
//
// On GNOME Wayland a client cannot position, raise or hide its own toplevel and
// xdg-shell has no "notification" window type, so the popup would land wherever
// mutter puts it and show up in the dock. This extension is the compositor-side
// half: the app asks (over D-Bus) for the popup to be placed / shown / hidden,
// and gets told when the user clicks outside it.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Mtk from 'gi://Mtk';
import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const APP_ID = 'dev.gustavosett.clipboard-history-gpui';
const BUS_NAME = 'dev.gustavosett.ClipboardHistoryGpui.Shell';
const OBJECT_PATH = '/dev/gustavosett/ClipboardHistoryGpui/Shell';
// The popup is told apart from Settings/Setup (same app id) by its title.
const POPUP_TITLE = 'Clipboard History Popup';

const IFACE = `
<node>
  <interface name="${BUS_NAME}">
    <method name="Place">
      <arg type="i" direction="in" name="x"/>
      <arg type="i" direction="in" name="y"/>
      <arg type="b" direction="out" name="ok"/>
    </method>
    <method name="SetVisible">
      <arg type="b" direction="in" name="visible"/>
      <arg type="b" direction="out" name="ok"/>
    </method>
    <method name="GetPointer">
      <arg type="i" direction="out" name="x"/>
      <arg type="i" direction="out" name="y"/>
    </method>
    <method name="GetMonitorAt">
      <arg type="i" direction="in" name="x"/>
      <arg type="i" direction="in" name="y"/>
      <arg type="(iiii)" direction="out" name="work_area"/>
    </method>
    <method name="ListWindows">
      <arg type="a(ssbiiiibb)" direction="out" name="windows"/>
    </method>
    <method name="Probe">
      <arg type="s" direction="out" name="report"/>
    </method>
    <method name="DebugMove">
      <arg type="i" direction="in" name="x"/>
      <arg type="i" direction="in" name="y"/>
    </method>
    <method name="DebugClick"/>
    <method name="DebugButton">
      <arg type="b" direction="in" name="pressed"/>
    </method>
    <method name="DebugKey">
      <arg type="u" direction="in" name="keyval"/>
    </method>
    <signal name="OutsideClick"/>
    <property name="Version" type="u" access="read"/>
  </interface>
</node>`;

export default class ClipboardHelper extends Extension {
    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._dbus.export(Gio.DBus.session, OBJECT_PATH);
        this._nameId = Gio.DBus.session.own_name(BUS_NAME, Gio.BusNameOwnerFlags.NONE, null, null);
        this._visible = false;
        this._pos = null;
        this._hideFromDock();
        this._watchWindows();
        this._captureId = global.stage.connect('captured-event', (_s, event) => this._onEvent(event));
    }

    disable() {
        if (this._captureId) global.stage.disconnect(this._captureId);
        this._captureId = 0;
        if (this._createdId) global.display.disconnect(this._createdId);
        if (this._mapId) global.window_manager.disconnect(this._mapId);
        this._createdId = this._mapId = 0;
        this._restoreDock();
        if (this._nameId) Gio.DBus.session.unown_name(this._nameId);
        this._nameId = 0;
        this._dbus?.unexport();
        this._dbus = null;
    }

    get Version() { return 1; }

    _isPopup(win) {
        return win.get_title() === POPUP_TITLE &&
            (win.get_gtk_application_id() === APP_ID || win.get_wm_class() === APP_ID);
    }

    // The permanent 1×1 holder window the app keeps to stay alive with no
    // visible window; it is as invisible to the user as the popup is while hidden.
    _isHolder(win) {
        const r = win.get_frame_rect();
        return !win.get_title() && r.width <= 2 && r.height <= 2 &&
            (win.get_gtk_application_id() === APP_ID || win.get_wm_class() === APP_ID);
    }

    _popup() {
        return global.get_window_actors().map(a => a.meta_window).find(w => this._isPopup(w)) ?? null;
    }

    // ---- D-Bus ---------------------------------------------------------

    // The app asks before the compositor has mapped its window (the very first
    // show happens in the same tick the window is created), so requests are
    // remembered and replayed when the popup appears.
    Place(x, y) {
        this._pos = [x, y];
        const win = this._popup();
        if (win) win.move_frame(true, x, y);
        return true;
    }

    SetVisible(visible) {
        this._visible = visible;
        const win = this._popup();
        if (win) this._apply(win);
        return true;
    }

    _apply(win) {
        if (this._visible) {
            if (this._pos) win.move_frame(true, this._pos[0], this._pos[1]);
            win.unminimize();
            win.stick();
            win.make_above();
            Main.activateWindow(win);
        } else {
            win.minimize();
        }
    }

    // A new popup window starts hidden unless the app already asked to show it.
    _watchWindows() {
        const adopt = win => {
            if (this._isPopup(win)) this._apply(win);
        };
        this._createdId = global.display.connect('window-created', (_d, win) => {
            adopt(win);
            // The title may be set after the window object exists.
            const id = win.connect('notify::title', () => {
                win.disconnect(id);
                adopt(win);
            });
        });
        this._mapId = global.window_manager.connect('map', (_wm, actor) => adopt(actor.meta_window));
    }

    GetPointer() {
        const [x, y] = global.get_pointer();
        return [x, y];
    }

    // Work area (monitor minus panels) of the monitor containing the point.
    GetMonitorAt(x, y) {
        const display = global.display;
        let index = display.get_monitor_index_for_rect(new Mtk.Rectangle({x, y, width: 1, height: 1}));
        if (index < 0) index = display.get_primary_monitor();
        const r = Main.layoutManager.getWorkAreaForMonitor(index);
        return [r.x, r.y, r.width, r.height];
    }

    ListWindows() {
        return global.get_window_actors().map(a => {
            const w = a.meta_window;
            const r = w.get_frame_rect();
            return [w.get_title() ?? '', w.get_wm_class() ?? '', w.is_skip_taskbar(),
                r.x, r.y, r.width, r.height, w.minimized, w.is_above()];
        });
    }

    Probe() {
        const win = this._popup();
        const app = win ? Shell.WindowTracker.get_default().get_window_app(win) : null;
        return JSON.stringify({
            shell: Config_version(),
            popup: !!win,
            app: app?.get_id() ?? null,
            appRunning: app ? app.state === Shell.AppState.RUNNING : null,
            type: win?.get_window_type(),
            skipTaskbar: win?.is_skip_taskbar(),
            focus: global.display.focus_window?.get_title() ?? null,
            sticky: win?.is_on_all_workspaces(),
            running: Shell.AppSystem.get_default().get_running().map(a => a.get_id()),
            runningUnfiltered: this._origGetRunning?.apply(Shell.AppSystem.get_default()).map(a => a.get_id()),
            dash: Main.overview.dash._box.get_children().map(c => c.child?.app?.get_id() ?? null),
        });
    }

    // Test hooks (headless CI): drive a virtual pointer. Only with
    // CLIPBOARD_HELPER_DEBUG=1 in the shell's environment.
    _debugDevice() {
        if (!GLib.getenv('CLIPBOARD_HELPER_DEBUG')) return null;
        this._vdev ??= Clutter.get_default_backend().get_default_seat()
            .create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
        return this._vdev;
    }

    DebugMove(x, y) {
        this._debugDevice()?.notify_absolute_motion(GLib.get_monotonic_time(), x, y);
    }

    DebugClick() {
        const dev = this._debugDevice();
        if (!dev) return;
        dev.notify_button(GLib.get_monotonic_time(), Clutter.BUTTON_PRIMARY, Clutter.ButtonState.PRESSED);
        dev.notify_button(GLib.get_monotonic_time() + 1000, Clutter.BUTTON_PRIMARY, Clutter.ButtonState.RELEASED);
    }

    DebugButton(pressed) {
        this._debugDevice()?.notify_button(GLib.get_monotonic_time(), Clutter.BUTTON_PRIMARY,
            pressed ? Clutter.ButtonState.PRESSED : Clutter.ButtonState.RELEASED);
    }

    DebugKey(keyval) {
        if (!GLib.getenv('CLIPBOARD_HELPER_DEBUG')) return;
        this._vkbd ??= Clutter.get_default_backend().get_default_seat()
            .create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        const t = GLib.get_monotonic_time();
        this._vkbd.notify_keyval(t, keyval, Clutter.KeyState.PRESSED);
        this._vkbd.notify_keyval(t + 1000, keyval, Clutter.KeyState.RELEASED);
    }

    // ---- outside click -------------------------------------------------

    _onEvent(event) {
        const type = event.type();
        if (type !== Clutter.EventType.BUTTON_PRESS && type !== Clutter.EventType.TOUCH_BEGIN)
            return Clutter.EVENT_PROPAGATE;
        const win = this._visible ? this._popup() : null;
        if (!win) return Clutter.EVENT_PROPAGATE;
        const [x, y] = event.get_coords();
        const r = win.get_frame_rect();
        const inside = x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;
        if (!inside)
            this._dbus?.emit_signal('OutsideClick', null);
        return Clutter.EVENT_PROPAGATE;
    }

    // ---- dock ----------------------------------------------------------

    // Docks (the stock dash, Dash to Dock, Ubuntu Dock, …) list
    // `Shell.AppSystem.get_running()`. Hide our app from that list while the
    // popup is its only window.
    _hideFromDock() {
        const sys = Shell.AppSystem.get_default();
        this._origGetRunning = sys.get_running;
        const self = this;
        sys.get_running = function (...args) {
            return self._origGetRunning.apply(this, args).filter(app => {
                const wins = app.get_windows();
                return !(wins.length && wins.every(w => self._isPopup(w) || self._isHolder(w)));
            });
        };
        sys.emit('installed-changed');
    }

    _restoreDock() {
        const sys = Shell.AppSystem.get_default();
        if (this._origGetRunning) sys.get_running = this._origGetRunning;
        this._origGetRunning = null;
    }
}

import * as Config from 'resource:///org/gnome/shell/misc/config.js';
function Config_version() { return Config.PACKAGE_VERSION; }
