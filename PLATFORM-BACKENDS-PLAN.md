# Rencana: pisah implementasi platform jadi 3 backend (x11, cosmic, gnome)

Status (2026-10-11): **Fase 0–5 dikerjakan** di branch `platform-backends`. Sisa pekerjaan ada di bagian "Hasil & sisa" di bawah.
Dibuat dari audit `gpui-app/src` (commit `2b1118d`).

## 1. Tujuan

Sekarang semua kode khusus platform tercampur di `main.rs`, `window_drag.rs`, `geometry.rs`, `click_watch.rs`,
dan percabangan `is_cosmic()` / `is_gnome()` / `session::is_wayland()`. Targetnya tiga folder implementasi,
sisanya tetap shared:

| Backend | Peran |
|---------|-------|
| `x11` | **Legacy, perilaku sekarang dipertahankan.** Paksa XWayland (`pin_display_backend_to_x11`) di sesi Wayland, posisi kursor + drag lewat x11rb. Jadi fallback untuk semua desktop yang belum punya backend sendiri (KDE, Sway, dll.) dan sesi X11 asli. |
| `cosmic` | Popup native Wayland lewat `wlr-layer-shell`. Tanpa XWayland, tanpa trik override-redirect. |
| `gnome` | Popup native Wayland sebagai jendela biasa, ditempatkan oleh **ekstensi GNOME Shell** ("penjaga posisi"). |

Syarat: tanpa regresi di `x11`, dan satu binary yang memilih backend saat runtime.

## 2. Apa yang dipakai sekarang (hasil audit)

Titik panggilan khusus platform, yang menentukan bentuk antarmuka backend:

| Kebutuhan | Kode sekarang | Dipakai di |
|-----------|---------------|-----------|
| Paksa XWayland sebelum gpui jalan | `pin_display_backend_to_x11()` | `main.rs:196` |
| Posisi kursor global | `geometry::cursor_position()` (xdotool, lalu x11rb) | `main.rs:253` (`initial_origin`) |
| Pindahkan popup | `window_drag::move_popup_to` | `main.rs:104` |
| Tampil/sembunyi popup (map/unmap) | `window_drag::set_popup_mapped` | `main.rs:117,400,899` |
| Cari window id popup | `window_drag::popup_window_id` | `main.rs:848,861` |
| Drag header popup | `window_drag::WindowDrag` + `POLL_INTERVAL` | `ui/popup.rs:1328` |
| Sembunyikan jendela holder 1x1 dari dock | `window_drag::unmanage_holder` | `main.rs:702` |
| Mode override-redirect khusus COSMIC | `unmanaged_popup_enabled()` | `window_drag.rs:340` |
| Bayangan jendela Settings (mutter + X11) | `window_drag::set_frame_extents` | `main.rs:487` |
| Tutup saat klik di luar (evdev) | `click_watch::spawn_click_watcher` | `main.rs:671` |
| Hotkey global (X11 grab) | `hotkey::register_hotkeys` | `main.rs:726` |
| Daftar shortcut ke desktop | `cosmic_shortcut`, `gnome_shortcut` | `main.rs:593`, `ui/wizard.rs:127`, `ui/settings.rs:320`, `settings.rs:152` |
| Fokus asli sebelum popup | `core::focus_manager` (X11) | `main.rs:120` |

## 3. Keputusan desain

### 3.1 Dua sumbu yang berbeda
Ada dua hal yang sering tertukar:
- **Backend jendela** (x11 / cosmic / gnome): cara popup dibuat, ditaruh, dan ditutup.
- **Desktop** (GNOME / COSMIC): tempat shortcut didaftarkan dan tema dibaca.

Backend `x11` yang jalan di GNOME lewat XWayland tetap harus mendaftarkan shortcut ke GNOME. Jadi
`cosmic_shortcut.rs` dan `gnome_shortcut.rs` **bukan** milik folder backend. Rekomendasi: pindahkan ke
`platform/shortcuts/{cosmic,gnome}.rs` (shared, dipilih menurut `XDG_CURRENT_DESKTOP`), dan folder backend
hanya mengurus jendela.

### 3.2 Pemilihan saat runtime
Satu binary, `platform::select()` dipanggil paling awal di `main()` (sebelum gpui, menggantikan
`pin_display_backend_to_x11`). Urutan:

1. `WIN11_CLIPBOARD_BACKEND=x11|cosmic|gnome` (override untuk debug/CI).
2. `XDG_CURRENT_DESKTOP` berisi `cosmic` dan sesi Wayland → `cosmic`.
3. `XDG_CURRENT_DESKTOP` berisi `gnome`, sesi Wayland, **dan ekstensi aktif** → `gnome`.
4. Selain itu → `x11` (butuh `DISPLAY`; kalau tidak ada, jalan di Wayland biasa seperti sekarang).

GNOME tanpa ekstensi otomatis jatuh ke `x11`, dan wizard menawarkan pasang ekstensi.

### 3.3 Bentuk antarmuka
Trait object statis (`&'static dyn PlatformBackend`), bukan `cfg`, karena satu binary harus bisa memilih.

```rust
pub trait PlatformBackend: Sync {
    fn name(&self) -> &'static str;
    /// Sebelum gpui init: env, pin backend, dsb.
    fn prepare_process(&self);
    fn window_kind(&self) -> gpui::WindowKind;            // PopUp / LayerShell
    fn cursor_position(&self) -> Option<(i32, i32)>;      // None = pakai bottom_center
    fn place_popup(&self, size: (f32, f32), x: i32, y: i32) -> Result<(), String>;
    fn set_popup_visible(&self, size: (f32, f32), visible: bool) -> Result<(), String>;
    fn begin_drag(&self) -> Option<Box<dyn PopupDrag>>;   // None = serahkan ke compositor
    fn unmanage_holder(&self) -> Result<(), String>;
    fn set_frame_extents(&self, title: &str, margin: u32) -> Result<(), String>;
    fn spawn_outside_click_watcher(&self, shared: Shared); // no-op bila compositor yang menangani
    fn register_global_hotkeys(&self) -> Option<Hotkeys>;  // x11 saja; lainnya None
    fn capabilities(&self) -> Capabilities;               // cursor_follow, drag, outside_click, ...
}
```

`capabilities()` dipakai UI (mis. sembunyikan opsi "follow mouse" kalau backend tidak mendukung), jadi
tidak ada lagi `if session::is_wayland()` yang tersebar.

## 4. Struktur folder target

```
gpui-app/src/
  platform/
    mod.rs            # trait PlatformBackend, Capabilities, select()
    x11/
      mod.rs          # impl PlatformBackend (perilaku sekarang)
      window.rs       # dari window_drag.rs (ConfigureWindow, map/unmap, drag, frame extents)
      pointer.rs      # dari geometry::cursor_position (xdotool + x11rb)
      click_watch.rs  # dari click_watch.rs (evdev)
      hotkey.rs       # dari hotkey.rs (global-hotkey, X11 grab)
    cosmic/
      mod.rs          # Fase 1: kode override-redirect yang diekstrak; Fase 4: layer-shell
      layer.rs        # (Fase 4) surface layer-shell, posisi kartu, dismiss
    gnome/
      mod.rs          # impl PlatformBackend, bicara ke ekstensi lewat D-Bus
      extension/      # ekstensi GNOME Shell (metadata.json, extension.js)
      install.rs      # salin + enable ekstensi, cek status
    shortcuts/
      mod.rs          # pilih menurut desktop
      cosmic.rs       # dari cosmic_shortcut.rs
      gnome.rs        # dari gnome_shortcut.rs
  geometry.rs         # tinggal matematika murni: MonitorRect, clamp_to_monitor, bottom_center, position_valid
  ui/ history.rs settings.rs theme*.rs gtk_theme.rs theme_watch.rs backend.rs app_state.rs
  instance.rs tray.rs pickers.rs   # tetap shared
```

`core/` (clipboard_manager, input_simulator/uinput, paste_sync, user_settings, session, dst.) tidak berubah.

## 5. Fase

### Fase 0 — Pasang antarmuka, nol perubahan perilaku
- Buat `platform/` dengan trait, `select()`, dan `x11` yang membungkus kode yang ada. **Semua desktop memakai `x11` dulu.**
- Pindahkan file sesuai tabel di bagian 4. Ganti pemanggilan langsung di `main.rs`, `ui/popup.rs`, `ui/wizard.rs`, `ui/settings.rs` dengan `platform::backend()`.
- Pisahkan bagian COSMIC dari `window_drag.rs` ke `platform/cosmic/mod.rs` (override-redirect, `unmanage_holder`, pencarian window di dalam frame WM). `cosmic` di fase ini masih XWayland, jadi perilakunya sama dengan commit `2b1118d`.
- Verifikasi: `cargo test` (tes geometri dan shortcut yang ada), lalu smoke test di GNOME Wayland (mesin dev) dan COSMIC: toggle, posisi kursor, drag, tutup saat klik di luar, paste. Hasilnya harus identik dengan sebelum refactor.
- **Selesai kalau:** tidak ada `if is_cosmic()` / `is_gnome()` / `is_wayland()` tersisa di luar `platform/`.

### Fase 1 — Spike ekstensi GNOME (menentukan apakah Fase 2 layak)
Buat ekstensi minimal dan buktikan satu per satu di GNOME Wayland (jangan asumsikan):

| # | Pertanyaan | Kemungkinan API |
|---|-----------|-----------------|
| S1 | Bisa memindahkan jendela ke koordinat absolut? | `Meta.Window.move_frame()` |
| S2 | Bisa membaca posisi kursor global? | `global.get_pointer()` |
| S3 | Bisa menyembunyikan popup dari dock/alt-tab? | **risiko terbesar.** Di Wayland native tidak ada petunjuk tipe jendela (beda dengan `NOTIFICATION` di X11), dan ekstensi tidak bisa mengubah tipe jendela. Satu-satunya jalan: memfilter jendela kita di dock-nya. Dash bawaan, Dash to Dock, Dash to Panel, dan Ubuntu Dock punya kode filter masing-masing, jadi harus dicek satu per satu |
| S4 | Bisa menjaga selalu di atas dan di semua workspace? | `make_above()`, `stick()` |
| S5 | Bisa mendeteksi klik di luar popup tanpa evdev? | `global.stage` captured-event / sinyal fokus |
| S6 | Bagaimana jendela dikenali? | `app_id` = `APP_ID`, cek `wm_class` |
| S7 | Kembali ke jendela sebelumnya setelah popup ditutup, agar paste (uinput) masuk ke jendela yang benar? | fokus otomatis dikembalikan compositor, perlu dicek |

**Keputusan:** kalau S3 hanya berhasil untuk sebagian dock, tentukan dock mana yang didukung (minimal dash bawaan
dan Dash to Dock), dan sisanya tampil di dock seperti jendela biasa. Kalau S3 gagal total, backend `gnome` ditunda
dan GNOME tetap di `x11`.

### Fase 2 — Backend `gnome`
- Ekstensi mengekspos antarmuka D-Bus kecil (mis. `Place(x, y, w, h)`, `SetVisible`, `GetPointer`) dan memberi
  sinyal `OutsideClick`. Aplikasi (`gnome/mod.rs`) memanggilnya lewat `zbus` (sudah jadi dependensi).
- Aplikasi berjalan di gpui Wayland native (`WindowKind::PopUp`), tanpa `pin_display_backend_to_x11`.
- Drag header: serahkan ke compositor (`start_window_move`), atau lewat ekstensi bila S1 menunjukkan itu perlu.
- `install.rs`: salin ekstensi ke `~/.local/share/gnome-shell/extensions/<uuid>/`, `gnome-extensions enable`,
  dan tampilkan status di wizard.
- **Catatan penting:** di Wayland, GNOME Shell tidak bisa di-restart, jadi ekstensi yang **baru dipasang** baru
  termuat setelah logout/login. Wizard harus menjelaskan ini, dan `select()` tetap jatuh ke `x11` sampai ekstensi aktif.
- Kompatibilitas: tentukan rentang `shell-version` (ekstensi ESM, GNOME 45+) dan uji di versi terendah dan tertinggi.

### Fase 3 — Naik dari `gpui =0.2.2` ke `gpui-pre` 0.3.8 (keputusan: dikerjakan)
`gpui 0.2.2` tidak punya `WindowKind::LayerShell`. `~/Project/gpui-wayland-patches` (basis `gpui-pre` 0.3.8)
sudah punya `layer_shell`, ditambah patch blur (`set_background_blur_regions`, `ext-background-effect-v1`).
- Migrasi seluruh `gpui-app` ke `gpui-pre` lewat `[patch.crates-io]` dengan `rev` yang dipin.
- Ini perubahan API besar dan menyentuh semua UI, jadi kerjakan di branch terpisah, **sebelum** logika backend baru.
- Verifikasi: seluruh UI dan tes jalan sama di backend `x11` dan `gnome`.
- Risiko: churn API `gpui-pre`, dan build lebih lama. Diputuskan tetap dilakukan, karena `gpui 0.2.2` sudah tertinggal
  dan `gpui-wayland-patches` sudah dipakai proyek lain (basis yang sama).

### Fase 4 — Backend `cosmic` (layer-shell)
Ide: satu surface layer-shell transparan **sebesar satu output penuh** di `Layer::Overlay`, dengan kartu popup digambar di dalamnya.
- Posisi kursor: event `wl_pointer.enter` pada surface itu memberi koordinat. Kartu digambar di sana dengan `clamp_to_monitor`. Sebelum event pertama, pakai `bottom_center`.
- Klik di luar kartu = tutup. Tidak butuh evdev, jadi grup `input` tidak lagi diperlukan di COSMIC.
- Drag = menggeser kartu di dalam surface (murni sisi klien, menggantikan polling X11 di `window_drag.rs`).
- Fokus keyboard: `KeyboardInteractivity::OnDemand` atau `Exclusive`, diputuskan saat uji.
- Tidak ada entri dock dan tidak ada title bar, jadi override-redirect, `unmanage_holder`, dan `COSMIC_UNMANAGED` dihapus.
- Blur: pakai patch `ext-background-effect-v1` dengan region sebesar kartu.
- Risiko: pilihan output saat surface dibuat (monitor tempat kursor berada belum diketahui sebelum map), dan perilaku IME/keyboard di layer overlay.
- Desain `layer.rs` dibuat tidak bergantung pada COSMIC, jadi nanti bisa dipakai KDE/Sway (folder bisa diganti nama jadi `layer_shell`).

### Fase 5 — Packaging, dokumentasi, CI
- AppImage membawa ekstensi GNOME (di `usr/share/...`); `install.rs` menyalinnya saat pertama jalan.
- Perbarui `BUILD-AND-SETUP.md`, `PACKAGING.md`, dan README (tabel dukungan per desktop).
- Tambahkan env `WIN11_CLIPBOARD_BACKEND` ke dokumentasi debug.
- Matriks uji manual di `.planning/phases/.../HUMAN-CHECKLIST.md`: GNOME Wayland (dengan dan tanpa ekstensi), COSMIC, GNOME/X11, KDE (fallback `x11`).

## 6. Urutan dan ketergantungan

```
Fase 0 ──┬─> Fase 1 ──> Fase 2          (gnome)
         └─> Fase 3 ──> Fase 4          (cosmic)
                              └──> Fase 5
```
Fase 1–2 dan Fase 3–4 bisa jalan paralel setelah Fase 0. Fase 0 adalah satu-satunya yang wajib dulu,
dan paling murah: nilainya langsung terasa walau fase lain tidak pernah dikerjakan.

## 7. Risiko dan keputusan terbuka

1. **Lokasi modul shortcut** (bagian 3.1): diputuskan shared di `platform/shortcuts/`.
2. **S3 (sembunyi dari dock di GNOME)** belum terbukti. Hasil spike menentukan nasib backend `gnome`.
3. **Ekstensi butuh relogin** pada pemasangan pertama di Wayland, dan bisa rusak di update GNOME.
4. **Migrasi `gpui-pre`** adalah pekerjaan terbesar, tapi sudah diputuskan dikerjakan (Fase 3).
5. **Fokus dan paste di Wayland native:** `core::focus_manager` bersifat X11. Di backend native, compositor
   mengembalikan fokus sendiri; perlu diuji bahwa urutan hide lalu paste (uinput) mendarat di jendela yang benar (S7).
6. **Verifikasi visual:** GNOME memblokir screenshot programatik dari output komposit (tercatat di `STATE.md`),
   jadi sebagian uji butuh mata manusia.
7. **Hotkey global:** hanya ada di `x11` (`global-hotkey`). Backend native tetap memakai shortcut tingkat desktop
   (`--toggle` lewat IPC `instance.rs`), jadi perilakunya tidak berubah.

## 8. Hasil & sisa (2026-10-11)

Diuji tanpa sesi grafis: `gnome-shell --headless` (50.4), `cosmic-comp` nested di dalamnya (grim untuk screenshot),
Xvfb + openbox untuk X11, pointer/keyboard virtual lewat metode debug ekstensi (`CLIPBOARD_HELPER_DEBUG=1`).

| Fase | Hasil |
|---|---|
| 0 | Selesai. `platform::{x11,cosmic,gnome,shortcuts}`, `platform::init()`. |
| 1 | S1–S7 terbukti di GNOME Shell 50.4: `move_frame` (S1), `global.get_pointer` (S2), filter `Shell.AppSystem.get_running()` menyembunyikan app dari dash bawaan selama hanya popup yang terbuka (S3), `stick`+`make_above` (S4), klik di luar lewat `captured-event` (S5), pengenal jendela lewat judul + app_id (S6), fokus kembali ke jendela sebelumnya setelah `minimize` (S7). S3 untuk Dash to Dock / Ubuntu Dock / Dash to Panel **belum diuji**. |
| 2 | Selesai. Ekstensi tertanam di binary (`include_str!`), dipasang wizard atau `--install-gnome-extension`; permintaan ditunda sampai jendela muncul (show pertama terjadi sebelum jendela dipetakan); tanpa jendela holder; Settings/Setup memakai `set_client_inset` (frame = ukuran kartu). Hanya Shell 50 yang diuji. |
| 3 | Selesai (dikerjakan subagent, lalu digabung). Catatan: `gpui-pre` keluar otomatis saat jendela terakhir ditutup, jadi dipasang `QuitMode::Explicit`. **Patch masih lewat path lokal** (`~/Project/gpui-wayland-patches`); harus jadi `git = …, rev = …` setelah repo itu di-push, kalau tidak CI/Docker gagal. |
| 4 | Selesai: overlay layer-shell per-show (dibuat saat tampil, dihancurkan saat sembunyi), kartu di posisi pointer, drag dan klik-luar di sisi klien, `KeyboardInteractivity::Exclusive`. Diuji di cosmic-comp nested. Blur (`ext-background-effect-v1`) ada tapi **opt-in** (`WIN11_CLIPBOARD_BLUR=1`) karena hasil visualnya belum bisa diperiksa. Kode override-redirect/XWayland COSMIC dihapus. Latensi pembuatan jendela per-show di GPU asli belum diukur (di sandbox ini 8–10 dtk karena render software dan beban mesin). |
| 5 | Dokumen (`PACKAGING.md`, `BUILD-AND-SETUP.md`, README), matriks uji manual (`.planning/phases/06-platform-backends/HUMAN-CHECKLIST.md`), env `WIN11_CLIPBOARD_BACKEND`. Ekstensi tertanam, jadi AppImage tidak perlu payload tambahan. Workflow CI belum diubah. |
