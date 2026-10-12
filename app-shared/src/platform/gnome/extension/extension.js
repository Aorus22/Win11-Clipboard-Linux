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
import St from 'gi://St';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

// The GPUI and iced frontends share this extension; both use the same popup title.
const APP_IDS = [
    'dev.gustavosett.clipboard-history-gpui',
    'dev.gustavosett.clipboard-history-iced',
];
const BUS_NAME = 'dev.gustavosett.ClipboardHistoryGpui.Shell';
const OBJECT_PATH = '/dev/gustavosett/ClipboardHistoryGpui/Shell';
// Single-instance sockets of the two frontends (`$XDG_RUNTIME_DIR/<name>`);
// writing a command word and a newline to one is what `<exe> --toggle` does.
const SOCKETS = ['win11-clipboard-gpui.sock', 'win11-clipboard-iced.sock'];
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
    <method name="DebugScroll">
      <arg type="d" direction="in" name="dy"/>
    </method>
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
        this._hiddenWindows = new Map();
        this._hideFromDock();
        this._watchWindows();
        for (const actor of global.get_window_actors()) {
            const win = actor.meta_window;
            if (this._isPopup(win) || this._isHolder(win)) this._hideFromWindowLists(win);
        }
        this._captureId = global.stage.connect('captured-event', (_s, event) => this._onEvent(event));
        this._addIndicator();
    }

    disable() {
        this._indicator?.destroy();
        this._indicator = null;
        if (this._captureId) global.stage.disconnect(this._captureId);
        this._captureId = 0;
        if (this._createdId) global.display.disconnect(this._createdId);
        if (this._mapId) global.window_manager.disconnect(this._mapId);
        this._createdId = this._mapId = 0;
        this._restoreWindowLists();
        this._restoreDock();
        if (this._nameId) Gio.DBus.session.unown_name(this._nameId);
        this._nameId = 0;
        this._dbus?.unexport();
        this._dbus = null;
    }

    get Version() { return 1; }

    _isPopup(win) {
        return win.get_title() === POPUP_TITLE &&
            (APP_IDS.includes(win.get_gtk_application_id()) || APP_IDS.includes(win.get_wm_class()));
    }

    // The permanent 1×1 holder window the app keeps to stay alive with no
    // visible window; it is as invisible to the user as the popup is while hidden.
    _isHolder(win) {
        const r = win.get_frame_rect();
        return !win.get_title() && r.width <= 2 && r.height <= 2 &&
            (APP_IDS.includes(win.get_gtk_application_id()) || APP_IDS.includes(win.get_wm_class()));
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
            // mutter refuses to minimize a skip-taskbar window ("cannot be
            // minimized, but something tried anyways"), and hiding the popup
            // from window lists sets exactly that. Lift the flag around the
            // call; both steps are synchronous, so no list ever sees it.
            const hidden = this._hiddenWindows.has(win);
            if (hidden) win.show_in_window_list();
            win.minimize();
            if (hidden) win.hide_from_window_list();
        }
    }

    _hideFromWindowLists(win) {
        if (this._hiddenWindows.has(win)) return;
        try {
            const alreadyHidden = win.is_skip_taskbar();
            if (!alreadyHidden) {
                win.hide_from_window_list();
                this._hiddenWindows.set(win, true);
            }
        } catch (error) {
            console.error(`[clipboard-history] could not hide popup from taskbars: ${error}`);
        }
    }

    _restoreWindowLists() {
        for (const win of this._hiddenWindows.keys()) {
            try { win.show_in_window_list(); } catch (_) { /* window already closed */ }
        }
        this._hiddenWindows.clear();
    }

    // A new popup window starts hidden unless the app already asked to show it.
    _watchWindows() {
        const adopt = win => {
            if (this._isPopup(win) || this._isHolder(win)) this._hideFromWindowLists(win);
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

    DebugScroll(dy) {
        this._debugDevice()?.notify_scroll_continuous(GLib.get_monotonic_time(), 0, dy,
            Clutter.ScrollSource.WHEEL, Clutter.ScrollFinishFlags.NONE);
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

    // ---- panel icon ----------------------------------------------------

    // Icon-only top-bar button with a small menu. GNOME has no system tray
    // (StatusNotifier icons need another extension), so the helper provides it.
    _addIndicator() {
        const button = new PanelMenu.Button(0.0, 'Clipboard History', false);
        button.add_child(new St.Icon({
            icon_name: 'edit-paste-symbolic',
            style_class: 'system-status-icon',
        }));
        button.menu.addAction('Show Clipboard', () => this._signalApp('toggle'));
        button.menu.addAction('Settings', () => this._signalApp('settings'));
        button.menu.addAction('Quit', () => this._signalApp('quit'));
        Main.panel.addToStatusArea('clipboard-history-gpui', button);
        this._indicator = button;
    }

    // Same words `<exe> --toggle|--settings-open|--quit` send over the
    // single-instance socket.
    _signalApp(word) {
        const runtime = GLib.get_user_runtime_dir();
        for (const name of SOCKETS) {
            try {
                const conn = new Gio.SocketClient().connect(
                    new Gio.UnixSocketAddress({path: `${runtime}/${name}`}), null);
                conn.get_output_stream().write_all(`${word}\n`, null);
                conn.close(null);
                return;
            } catch (_) { /* that frontend is not running */ }
        }
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
        const tracker = Shell.WindowTracker.get_default();
        this._origGetWindowApp = tracker.get_window_app;
        const self = this;
        sys.get_running = function (...args) {
            return self._origGetRunning.apply(this, args).filter(app => {
                const wins = app.get_windows();
                return !(wins.length && wins.every(w => self._isPopup(w) || self._isHolder(w)));
            });
        };
        tracker.get_window_app = function (win, ...args) {
            if (self._isPopup(win) || self._isHolder(win)) return null;
            return self._origGetWindowApp.apply(this, [win, ...args]);
        };
        sys.emit('installed-changed');
    }

    _restoreDock() {
        const sys = Shell.AppSystem.get_default();
        const tracker = Shell.WindowTracker.get_default();
        if (this._origGetRunning) sys.get_running = this._origGetRunning;
        if (this._origGetWindowApp) tracker.get_window_app = this._origGetWindowApp;
        this._origGetRunning = null;
        this._origGetWindowApp = null;
    }
}

import * as Config from 'resource:///org/gnome/shell/misc/config.js';
function Config_version() { return Config.PACKAGE_VERSION; }
